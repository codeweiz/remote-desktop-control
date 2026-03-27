//! IM Bridge — routes incoming IM messages to EventBus and throttles outgoing.
//!
//! Handles `im/on_message` and `im/on_status` notifications from the IM plugin,
//! subscribes to EventBus data events for monitored sessions, batches PTY output
//! (with ANSI stripping), and parses IM commands (`/sessions`, `/task add`, etc.)
//! to forward to the appropriate session or control plane.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

use rtb_core::event_bus::EventBus;
use rtb_core::events::{ControlEvent, DataEvent, SessionType};
use rtb_core::CoreState;

use crate::protocol::JsonRpcNotification;
use crate::types::{im_methods, ImConnectionStatus, ImOnMessageParams, ImOnStatusParams};

/// Default throttle interval for batching outgoing messages (5 seconds).
const DEFAULT_THROTTLE_MS: u64 = 5000;

/// Maximum length for agent tool result text before truncation.
const AGENT_TOOL_RESULT_MAX_LEN: usize = 1500;

/// Strips ANSI escape sequences from text.
fn strip_ansi(input: &str) -> String {
    // Match ANSI escape sequences: ESC[ ... final_byte
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check for CSI sequence (ESC [)
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                              // Skip until we hit a letter (final byte of CSI: 0x40-0x7E)
                loop {
                    match chars.next() {
                        Some(fc) if fc.is_ascii() && (0x40..=0x7E).contains(&(fc as u8)) => break,
                        None => break,
                        _ => continue,
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence (ESC ])
                chars.next();
                // Skip until BEL (0x07) or ST (ESC \)
                loop {
                    match chars.next() {
                        Some('\x07') => break,
                        Some('\x1b') => {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                                break;
                            }
                        }
                        None => break,
                        _ => continue,
                    }
                }
            }
            // Skip other single-char escape sequences
        } else if c == '\x07' || c == '\x0f' || c == '\x0e' {
            // Skip BEL, SI, SO
            continue;
        } else {
            result.push(c);
        }
    }

    result
}

/// A parsed IM command from a user message.
#[derive(Debug, Clone)]
pub enum ImCommand {
    /// `/new [provider]` — create new agent, auto-attach
    NewAgent { provider: String },
    /// `/list` — list agents with numbered index
    ListAgents,
    /// `/switch N` — switch to agent #N
    Switch { index: usize },
    /// `/help` — show commands
    Help,
    /// Plain text — forward to attached agent (auto-create if none)
    PlainText { text: String },
}

impl ImCommand {
    /// Parse a text message into an IM command.
    pub fn parse(text: &str) -> Self {
        let trimmed = text.trim();

        if !trimmed.starts_with('/') {
            return ImCommand::PlainText {
                text: trimmed.to_string(),
            };
        }

        let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "/new" => {
                let provider = if parts.len() >= 2 {
                    parts[1].to_string()
                } else {
                    "claude-code".to_string()
                };
                ImCommand::NewAgent { provider }
            }
            "/list" => ImCommand::ListAgents,
            "/switch" => {
                if parts.len() >= 2 {
                    if let Ok(idx) = parts[1].parse::<usize>() {
                        ImCommand::Switch { index: idx }
                    } else {
                        ImCommand::Help
                    }
                } else {
                    ImCommand::Help
                }
            }
            "/help" => ImCommand::Help,
            _ => ImCommand::PlainText {
                text: trimmed.to_string(),
            },
        }
    }
}

/// Sender handle for writing messages to the IM plugin's `send_message` method.
/// This is an async callback that the PluginManager provides after starting the plugin.
pub type ImPluginSender = Arc<
    dyn Fn(
            String,
            Option<String>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// Outgoing message with optional channel routing.
struct OutgoingMessage {
    text: String,
    channel: Option<String>,
}

/// Bridge between IM plugin notifications and the EventBus.
///
/// Responsibilities:
/// - Receive `im/on_message` from the IM plugin and dispatch as commands or PTY input
/// - Subscribe to EventBus data events for monitored sessions
/// - Batch PTY output (5s window), strip ANSI, send to IM plugin via `send_message`
/// - Track channel_id -> session_id mappings for routing
/// - Auto-create agent sessions when a user sends a message with no agent attached
pub struct ImBridge {
    core: Arc<CoreState>,
    /// Channel-to-session mapping: IM channel -> session ID.
    channel_sessions: Arc<Mutex<HashMap<String, String>>>,
    /// Outgoing message queue for throttled sending.
    outgoing_tx: mpsc::Sender<OutgoingMessage>,
    /// Throttle interval in milliseconds.
    _throttle_ms: u64,
    /// Sender handle for delivering messages to the IM plugin.
    plugin_sender: Arc<Mutex<Option<ImPluginSender>>>,
}

impl ImBridge {
    /// Create a new IM bridge.
    pub fn new(core: Arc<CoreState>) -> Self {
        Self::with_throttle(core, DEFAULT_THROTTLE_MS)
    }

    /// Create a new IM bridge with a custom throttle interval.
    pub fn with_throttle(core: Arc<CoreState>, throttle_ms: u64) -> Self {
        let plugin_sender: Arc<Mutex<Option<ImPluginSender>>> = Arc::new(Mutex::new(None));
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<OutgoingMessage>(256);

        // Start the throttled outgoing sender
        Self::start_throttle_task(outgoing_rx, throttle_ms, Arc::clone(&plugin_sender));

        Self {
            core,
            channel_sessions: Arc::new(Mutex::new(HashMap::new())),
            outgoing_tx,
            _throttle_ms: throttle_ms,
            plugin_sender,
        }
    }

    /// Set the plugin sender handle. Called by PluginManager after plugin is initialized.
    pub async fn set_plugin_sender(&self, sender: ImPluginSender) {
        let mut guard = self.plugin_sender.lock().await;
        *guard = Some(sender);
    }

    /// Start processing incoming notifications from the IM plugin.
    pub fn start(&self, mut notification_rx: mpsc::Receiver<JsonRpcNotification>) {
        let core = Arc::clone(&self.core);
        let channel_sessions = Arc::clone(&self.channel_sessions);
        let outgoing_tx = self.outgoing_tx.clone();

        tokio::spawn(async move {
            while let Some(notif) = notification_rx.recv().await {
                match notif.method.as_str() {
                    im_methods::ON_MESSAGE => {
                        if let Some(params) = notif.params {
                            match serde_json::from_value::<ImOnMessageParams>(params) {
                                Ok(msg) => {
                                    let clean_text = strip_ansi(&msg.text);
                                    debug!(
                                        sender = %msg.sender,
                                        channel = ?msg.channel,
                                        text = %clean_text,
                                        "received IM message"
                                    );

                                    let cmd = ImCommand::parse(&clean_text);
                                    Self::handle_command(
                                        cmd,
                                        msg.channel.clone(),
                                        &core,
                                        &channel_sessions,
                                        &outgoing_tx,
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    warn!(error = %e, "failed to parse im/on_message params");
                                }
                            }
                        }
                    }
                    im_methods::ON_STATUS => {
                        if let Some(params) = notif.params {
                            match serde_json::from_value::<ImOnStatusParams>(params) {
                                Ok(status) => {
                                    debug!(status = ?status.status, "IM status changed");
                                    match status.status {
                                        ImConnectionStatus::Connected => {
                                            info!("IM plugin connected");
                                        }
                                        ImConnectionStatus::Error
                                        | ImConnectionStatus::Disconnected => {
                                            core.event_bus.publish_control(
                                                ControlEvent::PluginError {
                                                    plugin_id: "im".to_string(),
                                                    error: status.message.unwrap_or_else(|| {
                                                        format!("{:?}", status.status)
                                                    }),
                                                },
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "failed to parse im/on_status params");
                                }
                            }
                        }
                    }
                    other => {
                        debug!(method = %other, "unknown IM notification method");
                    }
                }
            }
        });
    }

    /// Handle a parsed IM command.
    async fn handle_command(
        cmd: ImCommand,
        channel: Option<String>,
        core: &CoreState,
        channel_sessions: &Mutex<HashMap<String, String>>,
        outgoing_tx: &mpsc::Sender<OutgoingMessage>,
    ) {
        match cmd {
            ImCommand::NewAgent { provider } => {
                let session_id = nanoid::nanoid!(10);
                let name = format!("IM-Agent-{}", &session_id[..4]);
                let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));

                let reply = match core
                    .agent_manager
                    .create_agent(session_id.clone(), &name, &provider, "", cwd)
                    .await
                {
                    Ok(()) => {
                        if let Some(ch) = channel.as_ref() {
                            channel_sessions
                                .lock()
                                .await
                                .insert(ch.clone(), session_id.clone());
                            // Start monitoring agent output for this channel
                            Self::spawn_channel_monitor(
                                &core.event_bus,
                                &session_id,
                                ch,
                                outgoing_tx.clone(),
                            );
                        }
                        format!("Agent created: {name}\nYou can start chatting now.")
                    }
                    Err(e) => format!("Failed to create agent: {e}"),
                };
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: reply,
                        channel,
                    })
                    .await;
            }
            ImCommand::ListAgents => {
                let agents = core.agent_manager.list_agents();
                let current_session = if let Some(ch) = channel.as_ref() {
                    channel_sessions.lock().await.get(ch).cloned()
                } else {
                    None
                };

                let reply = if agents.is_empty() {
                    "No agents. Send a message to auto-create one, or use /new".to_string()
                } else {
                    let mut lines = vec!["Agents:".to_string()];
                    for (i, (sid, name, status, _created)) in agents.iter().enumerate() {
                        let current = if Some(sid) == current_session.as_ref() {
                            " <-"
                        } else {
                            ""
                        };
                        lines.push(format!("  #{} {} [{:?}]{}", i + 1, name, status, current));
                    }
                    lines.push("\nUse /switch N to switch.".to_string());
                    lines.join("\n")
                };
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: reply,
                        channel,
                    })
                    .await;
            }
            ImCommand::Switch { index } => {
                let agents = core.agent_manager.list_agents();
                let reply = if index == 0 || index > agents.len() {
                    format!(
                        "Invalid index. Use /list to see agents (1-{}).",
                        agents.len()
                    )
                } else {
                    let (sid, name, _, _) = &agents[index - 1];
                    if let Some(ch) = channel.as_ref() {
                        channel_sessions
                            .lock()
                            .await
                            .insert(ch.clone(), sid.clone());
                        // Start monitoring the switched-to agent's output
                        Self::spawn_channel_monitor(&core.event_bus, sid, ch, outgoing_tx.clone());
                    }
                    format!("Switched to {name}")
                };
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: reply,
                        channel,
                    })
                    .await;
            }
            ImCommand::Help => {
                let help = concat!(
                    "Commands:\n",
                    "  /new [provider] — create new agent (default: claude-code)\n",
                    "  /list — list agents\n",
                    "  /switch N — switch to agent #N\n",
                    "  /help — show this help\n",
                    "\nSend any text to chat with the current agent.\n",
                    "If no agent exists, one will be created automatically.",
                );
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: help.to_string(),
                        channel,
                    })
                    .await;
            }
            ImCommand::PlainText { text } => {
                if let Some(ch) = channel.as_ref() {
                    let session_id = {
                        let sessions = channel_sessions.lock().await;
                        sessions.get(ch).cloned()
                    };

                    let session_id = if let Some(sid) = session_id {
                        sid
                    } else {
                        // Auto-create agent
                        let sid = nanoid::nanoid!(10);
                        let name = format!("IM-Agent-{}", &sid[..4]);
                        let cwd = std::env::current_dir()
                            .unwrap_or_else(|_| std::path::PathBuf::from("/"));

                        match core
                            .agent_manager
                            .create_agent(sid.clone(), &name, "claude-code", "", cwd)
                            .await
                        {
                            Ok(()) => {
                                channel_sessions
                                    .lock()
                                    .await
                                    .insert(ch.clone(), sid.clone());
                                // Start monitoring agent output for this channel
                                Self::spawn_channel_monitor(
                                    &core.event_bus,
                                    &sid,
                                    ch,
                                    outgoing_tx.clone(),
                                );
                                let _ = outgoing_tx
                                    .send(OutgoingMessage {
                                        text: format!("Auto-created agent: {name}"),
                                        channel: Some(ch.clone()),
                                    })
                                    .await;
                                sid
                            }
                            Err(e) => {
                                let _ = outgoing_tx
                                    .send(OutgoingMessage {
                                        text: format!("Failed to create agent: {e}"),
                                        channel: Some(ch.clone()),
                                    })
                                    .await;
                                return;
                            }
                        }
                    };

                    // Forward message to agent (with source tracking)
                    if let Err(e) = core
                        .agent_manager
                        .send_message_from(&session_id, text, "feishu")
                        .await
                    {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("Failed to send to agent: {e}"),
                                channel: Some(ch.clone()),
                            })
                            .await;
                    }
                }
            }
        }
    }

    /// Subscribe to control events and forward notification triggers to IM.
    ///
    /// Spawns a background task that listens for `NotificationTriggered` control
    /// events and sends them as IM messages to all active channels.
    ///
    /// Also auto-monitors new agent sessions so their output is forwarded to
    /// all connected IM channels without requiring an explicit `/attach`.
    pub fn start_notification_listener(&self) {
        let mut control_rx = self.core.event_bus.subscribe_control();
        let outgoing_tx = self.outgoing_tx.clone();
        let event_bus = Arc::clone(&self.core.event_bus);

        tokio::spawn(async move {
            loop {
                match control_rx.recv().await {
                    Ok(event) => {
                        match event.as_ref() {
                            ControlEvent::NotificationTriggered {
                                session_id,
                                trigger_type,
                                summary,
                                urgent,
                            } => {
                                let urgency = if *urgent { " [URGENT]" } else { "" };
                                let text = format!(
                                    "[{trigger_type}]{urgency} session={session_id}: {summary}"
                                );
                                debug!(text = %text, "forwarding notification to IM");
                                let _ = outgoing_tx
                                    .send(OutgoingMessage {
                                        text,
                                        channel: None, // broadcast to default channel
                                    })
                                    .await;
                            }
                            ControlEvent::SessionCreated {
                                session_id,
                                session_type: SessionType::Agent,
                            } => {
                                info!(
                                    session_id = %session_id,
                                    "auto-monitoring new agent session for IM"
                                );
                                // Notify all IM channels about the new agent session
                                let _ = outgoing_tx
                                    .send(OutgoingMessage {
                                        text: format!(
                                            "\u{1f916} New agent session created: {session_id}"
                                        ),
                                        channel: None,
                                    })
                                    .await;
                                // Auto-monitor agent session, broadcasting to all channels
                                Self::spawn_broadcast_monitor(
                                    &event_bus,
                                    session_id,
                                    outgoing_tx.clone(),
                                );
                            }
                            _ => {}
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "IM notification listener lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });
    }

    /// Spawn a monitor for an agent session that sends output to a specific IM channel.
    fn spawn_channel_monitor(
        event_bus: &EventBus,
        session_id: &str,
        channel_id: &str,
        outgoing_tx: mpsc::Sender<OutgoingMessage>,
    ) {
        let mut data_rx = event_bus.create_data_subscriber(session_id);
        let _session_id = session_id.to_string();
        let channel_id = channel_id.to_string();

        tokio::spawn(async move {
            loop {
                match data_rx.recv().await {
                    Some(DataEvent::AgentText { content, .. }) => {
                        if !content.is_empty() {
                            let _ = outgoing_tx
                                .send(OutgoingMessage {
                                    text: content,
                                    channel: Some(channel_id.clone()),
                                })
                                .await;
                        }
                    }
                    Some(DataEvent::AgentUserMessage { .. }) => {}
                    Some(DataEvent::AgentThinking { .. }) => {}
                    Some(DataEvent::AgentToolUse { name, .. }) => {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("\u{1f527} {name}"),
                                channel: Some(channel_id.clone()),
                            })
                            .await;
                    }
                    Some(DataEvent::AgentToolResult {
                        output, is_error, ..
                    }) => {
                        let prefix = if is_error { "Error" } else { "Result" };
                        let truncated = if output.len() > AGENT_TOOL_RESULT_MAX_LEN {
                            format!("{}...[truncated]", &output[..AGENT_TOOL_RESULT_MAX_LEN])
                        } else {
                            output
                        };
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("{prefix}: {truncated}"),
                                channel: Some(channel_id.clone()),
                            })
                            .await;
                    }
                    Some(DataEvent::AgentProgress { message, .. }) => {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("[progress] {message}"),
                                channel: Some(channel_id.clone()),
                            })
                            .await;
                    }
                    Some(DataEvent::AgentTurnComplete { cost_usd, .. }) => {
                        let cost_info = cost_usd.map(|c| format!(" (${c:.4})")).unwrap_or_default();
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("\u{2705} Done{cost_info}"),
                                channel: Some(channel_id.clone()),
                            })
                            .await;
                    }
                    Some(DataEvent::AgentError {
                        message, guidance, ..
                    }) => {
                        let text = if guidance.is_empty() {
                            format!("\u{274c} {message}")
                        } else {
                            format!("\u{274c} {message}\n{guidance}")
                        };
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text,
                                channel: Some(channel_id.clone()),
                            })
                            .await;
                    }
                    Some(DataEvent::PtyOutput { .. }) | Some(DataEvent::PtyExited { .. }) => {}
                    None => return,
                }
            }
        });
    }

    /// Spawn a broadcast monitor for an agent session that sends output to ALL
    /// connected IM channels (channel: None), rather than a specific channel.
    fn spawn_broadcast_monitor(
        event_bus: &EventBus,
        session_id: &str,
        outgoing_tx: mpsc::Sender<OutgoingMessage>,
    ) {
        let mut data_rx = event_bus.create_data_subscriber(session_id);
        let session_id = session_id.to_string();

        tokio::spawn(async move {
            loop {
                match data_rx.recv().await {
                    Some(DataEvent::AgentText { content, .. }) => {
                        if !content.is_empty() {
                            let _ = outgoing_tx
                                .send(OutgoingMessage {
                                    text: format!("[agent {session_id}] {content}"),
                                    channel: None,
                                })
                                .await;
                        }
                    }
                    Some(DataEvent::AgentUserMessage { .. }) => {}
                    Some(DataEvent::AgentThinking { .. }) => {
                        // Skip thinking events to reduce noise
                    }
                    Some(DataEvent::AgentToolUse { name, .. }) => {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("[agent {session_id}] \u{1f527} Using tool: {name}"),
                                channel: None,
                            })
                            .await;
                    }
                    Some(DataEvent::AgentToolResult {
                        output, is_error, ..
                    }) => {
                        let prefix = if is_error {
                            "Tool error"
                        } else {
                            "Tool result"
                        };
                        let truncated = if output.len() > AGENT_TOOL_RESULT_MAX_LEN {
                            format!(
                                "{}...\n[truncated, {} bytes total]",
                                &output[..AGENT_TOOL_RESULT_MAX_LEN],
                                output.len()
                            )
                        } else {
                            output
                        };
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("[agent {session_id}] {prefix}: {truncated}"),
                                channel: None,
                            })
                            .await;
                    }
                    Some(DataEvent::AgentProgress { message, .. }) => {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!("[agent {session_id}] [progress] {message}"),
                                channel: None,
                            })
                            .await;
                    }
                    Some(DataEvent::AgentTurnComplete { cost_usd, .. }) => {
                        let cost_info = cost_usd
                            .map(|c| format!(" (cost: ${c:.4})"))
                            .unwrap_or_default();
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!(
                                    "[agent {session_id}] \u{2705} Turn complete{cost_info}"
                                ),
                                channel: None,
                            })
                            .await;
                    }
                    Some(DataEvent::AgentError {
                        message, guidance, ..
                    }) => {
                        let text = if guidance.is_empty() {
                            format!("[agent {session_id}] \u{274c} Error: {message}")
                        } else {
                            format!("[agent {session_id}] \u{274c} Error: {message}\nGuidance: {guidance}")
                        };
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text,
                                channel: None,
                            })
                            .await;
                    }
                    // PTY events from agent sessions (if any)
                    Some(DataEvent::PtyOutput { data, .. }) => {
                        let text = String::from_utf8_lossy(&data);
                        let clean = strip_ansi(&text);
                        if !clean.is_empty() {
                            let _ = outgoing_tx
                                .send(OutgoingMessage {
                                    text: format!("[agent {session_id}] {clean}"),
                                    channel: None,
                                })
                                .await;
                        }
                    }
                    Some(DataEvent::PtyExited { exit_code }) => {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: format!(
                                    "[agent {session_id}] PTY exited with code {exit_code}"
                                ),
                                channel: None,
                            })
                            .await;
                        return;
                    }
                    None => return, // channel closed
                }
            }
        });
    }

    /// Subscribe to a session's data events and forward PTY output to the IM channel.
    ///
    /// Spawns a background task that reads data events from the session,
    /// strips ANSI codes, and queues batched messages for the IM plugin.
    pub fn monitor_session(&self, session_id: &str, channel_id: &str) {
        let mut data_rx = self.core.event_bus.create_data_subscriber(session_id);
        let outgoing_tx = self.outgoing_tx.clone();
        let channel_id = channel_id.to_string();
        let session_id = session_id.to_string();

        tokio::spawn(async move {
            let mut pty_buffer = String::new();
            let flush_interval = tokio::time::Duration::from_secs(5);

            loop {
                let deadline = tokio::time::sleep(flush_interval);
                tokio::pin!(deadline);

                loop {
                    tokio::select! {
                        event = data_rx.recv() => {
                            match event {
                                // ── PTY events (unchanged) ──────────────────
                                Some(DataEvent::PtyOutput { data, .. }) => {
                                    let text = String::from_utf8_lossy(&data);
                                    let clean = strip_ansi(&text);
                                    pty_buffer.push_str(&clean);
                                }
                                Some(DataEvent::PtyExited { exit_code }) => {
                                    // Flush remaining buffer then notify
                                    if !pty_buffer.is_empty() {
                                        let _ = outgoing_tx.send(OutgoingMessage {
                                            text: std::mem::take(&mut pty_buffer),
                                            channel: Some(channel_id.clone()),
                                        }).await;
                                    }
                                    let _ = outgoing_tx.send(OutgoingMessage {
                                        text: format!("[session {session_id}] PTY exited with code {exit_code}"),
                                        channel: Some(channel_id.clone()),
                                    }).await;
                                    return;
                                }

                                // ── Agent events (Phase 2) ──────────────────
                                // Agent text: send immediately (low frequency)
                                Some(DataEvent::AgentText { content, .. }) => {
                                    if !content.is_empty() {
                                        let _ = outgoing_tx.send(OutgoingMessage {
                                            text: content,
                                            channel: Some(channel_id.clone()),
                                        }).await;
                                    }
                                }
                                Some(DataEvent::AgentUserMessage { .. }) => {}
                                // Agent thinking: skip to reduce noise
                                Some(DataEvent::AgentThinking { .. }) => {
                                    debug!(session_id = %session_id, "agent thinking event (skipped for IM)");
                                }
                                // Agent tool use: send formatted tool name
                                Some(DataEvent::AgentToolUse { name, .. }) => {
                                    let text = format!("\u{1f527} Using tool: {name}");
                                    let _ = outgoing_tx.send(OutgoingMessage {
                                        text,
                                        channel: Some(channel_id.clone()),
                                    }).await;
                                }
                                // Agent tool result: send truncated output
                                Some(DataEvent::AgentToolResult { output, is_error, .. }) => {
                                    let prefix = if is_error { "Tool error" } else { "Tool result" };
                                    let truncated = if output.len() > AGENT_TOOL_RESULT_MAX_LEN {
                                        format!(
                                            "{}...\n[truncated, {} bytes total]",
                                            &output[..AGENT_TOOL_RESULT_MAX_LEN],
                                            output.len()
                                        )
                                    } else {
                                        output
                                    };
                                    let text = format!("{prefix}: {truncated}");
                                    let _ = outgoing_tx.send(OutgoingMessage {
                                        text,
                                        channel: Some(channel_id.clone()),
                                    }).await;
                                }
                                // Agent progress: send update
                                Some(DataEvent::AgentProgress { message, .. }) => {
                                    let _ = outgoing_tx.send(OutgoingMessage {
                                        text: format!("[progress] {message}"),
                                        channel: Some(channel_id.clone()),
                                    }).await;
                                }
                                // Agent turn complete: send completion marker
                                Some(DataEvent::AgentTurnComplete { cost_usd, .. }) => {
                                    let cost_info = cost_usd
                                        .map(|c| format!(" (cost: ${c:.4})"))
                                        .unwrap_or_default();
                                    let text = format!("\u{2705} Turn complete{cost_info}");
                                    let _ = outgoing_tx.send(OutgoingMessage {
                                        text,
                                        channel: Some(channel_id.clone()),
                                    }).await;
                                }
                                // Agent error: send with guidance
                                Some(DataEvent::AgentError { message, guidance, .. }) => {
                                    let text = if guidance.is_empty() {
                                        format!("\u{274c} Agent error: {message}")
                                    } else {
                                        format!("\u{274c} Agent error: {message}\nGuidance: {guidance}")
                                    };
                                    let _ = outgoing_tx.send(OutgoingMessage {
                                        text,
                                        channel: Some(channel_id.clone()),
                                    }).await;
                                }

                                None => return, // channel closed
                            }
                        }
                        _ = &mut deadline => {
                            break;
                        }
                    }
                }

                // Flush accumulated PTY output
                if !pty_buffer.is_empty() {
                    let text = std::mem::take(&mut pty_buffer);
                    // Truncate very long output
                    let text = if text.len() > 4000 {
                        format!(
                            "{}...\n[truncated, {} bytes total]",
                            &text[..4000],
                            text.len()
                        )
                    } else {
                        text
                    };
                    let _ = outgoing_tx
                        .send(OutgoingMessage {
                            text,
                            channel: Some(channel_id.clone()),
                        })
                        .await;
                }
            }
        });
    }

    /// Queue a message for throttled outgoing delivery.
    pub async fn queue_outgoing(&self, text: String, channel: Option<String>) {
        let clean = strip_ansi(&text);
        if self
            .outgoing_tx
            .send(OutgoingMessage {
                text: clean,
                channel,
            })
            .await
            .is_err()
        {
            warn!("outgoing message channel closed");
        }
    }

    /// Background task that batches outgoing messages and sends them via the plugin sender.
    fn start_throttle_task(
        mut rx: mpsc::Receiver<OutgoingMessage>,
        throttle_ms: u64,
        plugin_sender: Arc<Mutex<Option<ImPluginSender>>>,
    ) {
        tokio::spawn(async move {
            let interval = tokio::time::Duration::from_millis(throttle_ms);
            // Batch per channel: channel_id -> accumulated texts
            let mut batches: HashMap<Option<String>, Vec<String>> = HashMap::new();

            loop {
                // Collect messages for the throttle interval
                let deadline = tokio::time::sleep(interval);
                tokio::pin!(deadline);

                loop {
                    tokio::select! {
                        msg = rx.recv() => {
                            match msg {
                                Some(out) => {
                                    batches.entry(out.channel).or_default().push(out.text);
                                }
                                None => return, // channel closed
                            }
                        }
                        _ = &mut deadline => {
                            break;
                        }
                    }
                }

                if batches.is_empty() {
                    continue;
                }

                // Flush all channel batches
                let sender_guard = plugin_sender.lock().await;
                for (channel, texts) in batches.drain() {
                    let combined = texts.join("\n---\n");
                    debug!(
                        channel = ?channel,
                        messages = texts.len(),
                        total_len = combined.len(),
                        "flushing batched outgoing messages"
                    );

                    if let Some(ref sender) = *sender_guard {
                        sender(combined, channel).await;
                    } else {
                        debug!("no plugin sender set, dropping outgoing message batch");
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_basic() {
        assert_eq!(strip_ansi("hello"), "hello");
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[1;32mbold green\x1b[0m"), "bold green");
    }

    #[test]
    fn test_strip_ansi_complex() {
        // Cursor movement
        assert_eq!(strip_ansi("\x1b[2Jcleared"), "cleared");
        // Mixed content
        assert_eq!(
            strip_ansi("before\x1b[31mred\x1b[0mafter"),
            "beforeredafter"
        );
    }

    #[test]
    fn test_strip_ansi_empty() {
        assert_eq!(strip_ansi(""), "");
        assert_eq!(strip_ansi("\x1b[m"), "");
    }

    #[test]
    fn test_parse_command_plain_text() {
        match ImCommand::parse("hello world") {
            ImCommand::PlainText { text } => assert_eq!(text, "hello world"),
            other => panic!("expected PlainText, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_help() {
        match ImCommand::parse("/help") {
            ImCommand::Help => {}
            other => panic!("expected Help, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_new_default() {
        match ImCommand::parse("/new") {
            ImCommand::NewAgent { provider } => assert_eq!(provider, "claude-code"),
            other => panic!("expected NewAgent, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_new_provider() {
        match ImCommand::parse("/new openai") {
            ImCommand::NewAgent { provider } => assert_eq!(provider, "openai"),
            other => panic!("expected NewAgent, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_list() {
        match ImCommand::parse("/list") {
            ImCommand::ListAgents => {}
            other => panic!("expected ListAgents, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_switch() {
        match ImCommand::parse("/switch 2") {
            ImCommand::Switch { index } => assert_eq!(index, 2),
            other => panic!("expected Switch, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_switch_invalid() {
        match ImCommand::parse("/switch abc") {
            ImCommand::Help => {}
            other => panic!("expected Help for invalid switch, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_unknown_slash() {
        match ImCommand::parse("/unknown foo") {
            ImCommand::PlainText { text } => assert_eq!(text, "/unknown foo"),
            other => panic!("expected PlainText for unknown command, got {other:?}"),
        }
    }
}
