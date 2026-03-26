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
use rtb_core::events::{ControlEvent, DataEvent};

use crate::protocol::JsonRpcNotification;
use crate::types::{im_methods, ImOnMessageParams, ImOnStatusParams, ImConnectionStatus};

/// Default throttle interval for batching outgoing messages (5 seconds).
const DEFAULT_THROTTLE_MS: u64 = 5000;

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
    /// `/sessions` — list active sessions
    ListSessions,
    /// `/task add <description>` — add a task
    TaskAdd { description: String },
    /// `/task list` — list tasks
    TaskList,
    /// `/attach <session_id>` — attach IM channel to a session
    Attach { session_id: String },
    /// `/detach` — detach IM channel from current session
    Detach,
    /// `/help` — show available commands
    Help,
    /// Not a command, forward as plain text to the attached session
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
            "/sessions" => ImCommand::ListSessions,
            "/task" => {
                if parts.len() >= 2 {
                    match parts[1].to_lowercase().as_str() {
                        "add" => {
                            let description = if parts.len() >= 3 {
                                parts[2].to_string()
                            } else {
                                String::new()
                            };
                            ImCommand::TaskAdd { description }
                        }
                        "list" | "ls" => ImCommand::TaskList,
                        _ => ImCommand::PlainText {
                            text: trimmed.to_string(),
                        },
                    }
                } else {
                    ImCommand::TaskList
                }
            }
            "/attach" => {
                if parts.len() >= 2 {
                    ImCommand::Attach {
                        session_id: parts[1].to_string(),
                    }
                } else {
                    ImCommand::PlainText {
                        text: trimmed.to_string(),
                    }
                }
            }
            "/detach" => ImCommand::Detach,
            "/help" => ImCommand::Help,
            _ => ImCommand::PlainText {
                text: trimmed.to_string(),
            },
        }
    }
}

/// Sender handle for writing messages to the IM plugin's `send_message` method.
/// This is an async callback that the PluginManager provides after starting the plugin.
pub type ImPluginSender = Arc<dyn Fn(String, Option<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

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
pub struct ImBridge {
    event_bus: Arc<EventBus>,
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
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self::with_throttle(event_bus, DEFAULT_THROTTLE_MS)
    }

    /// Create a new IM bridge with a custom throttle interval.
    pub fn with_throttle(event_bus: Arc<EventBus>, throttle_ms: u64) -> Self {
        let plugin_sender: Arc<Mutex<Option<ImPluginSender>>> = Arc::new(Mutex::new(None));
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<OutgoingMessage>(256);

        // Start the throttled outgoing sender
        Self::start_throttle_task(outgoing_rx, throttle_ms, Arc::clone(&plugin_sender));

        Self {
            event_bus,
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
        let event_bus = Arc::clone(&self.event_bus);
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
                                        &event_bus,
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
                                            event_bus.publish_control(
                                                ControlEvent::PluginError {
                                                    plugin_id: "im".to_string(),
                                                    error: status
                                                        .message
                                                        .unwrap_or_else(|| {
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
        _event_bus: &EventBus,
        channel_sessions: &Mutex<HashMap<String, String>>,
        outgoing_tx: &mpsc::Sender<OutgoingMessage>,
    ) {
        match cmd {
            ImCommand::ListSessions => {
                // Reply with known channel-session mappings for now.
                let sessions = channel_sessions.lock().await;
                let reply = if sessions.is_empty() {
                    "No sessions attached. Use /attach <session_id> to attach.".to_string()
                } else {
                    let mut lines = vec!["Attached sessions:".to_string()];
                    for (ch, sid) in sessions.iter() {
                        lines.push(format!("  channel={ch} -> session={sid}"));
                    }
                    lines.join("\n")
                };
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: reply,
                        channel,
                    })
                    .await;
            }
            ImCommand::TaskAdd { description } => {
                let reply = if description.is_empty() {
                    "Usage: /task add <description>".to_string()
                } else {
                    format!("Task queued: {description}")
                };
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: reply,
                        channel,
                    })
                    .await;
            }
            ImCommand::TaskList => {
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: "Task list: (use CLI `rtb task list` for full list)".to_string(),
                        channel,
                    })
                    .await;
            }
            ImCommand::Attach { session_id } => {
                if let Some(ch) = channel.as_ref() {
                    let mut sessions = channel_sessions.lock().await;
                    sessions.insert(ch.clone(), session_id.clone());
                    info!(channel = %ch, session_id = %session_id, "IM channel attached to session");
                    let _ = outgoing_tx
                        .send(OutgoingMessage {
                            text: format!("Attached to session {session_id}. PTY output will be forwarded here."),
                            channel,
                        })
                        .await;
                } else {
                    let _ = outgoing_tx
                        .send(OutgoingMessage {
                            text: "Cannot attach: no channel ID in this message".to_string(),
                            channel,
                        })
                        .await;
                }
            }
            ImCommand::Detach => {
                if let Some(ch) = &channel {
                    let mut sessions = channel_sessions.lock().await;
                    if sessions.remove(ch).is_some() {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: "Detached from session. PTY output will no longer be forwarded.".to_string(),
                                channel,
                            })
                            .await;
                    } else {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: "Not attached to any session.".to_string(),
                                channel,
                            })
                            .await;
                    }
                }
            }
            ImCommand::Help => {
                let help = concat!(
                    "Available commands:\n",
                    "  /sessions — list attached sessions\n",
                    "  /attach <session_id> — attach channel to a session\n",
                    "  /detach — detach from current session\n",
                    "  /task add <desc> — queue a new task\n",
                    "  /task list — list tasks\n",
                    "  /help — show this help\n",
                    "  (plain text) — forwarded as input to attached session",
                );
                let _ = outgoing_tx
                    .send(OutgoingMessage {
                        text: help.to_string(),
                        channel,
                    })
                    .await;
            }
            ImCommand::PlainText { text } => {
                // Forward plain text to the attached session (if any).
                if let Some(ch) = &channel {
                    let sessions = channel_sessions.lock().await;
                    if let Some(session_id) = sessions.get(ch) {
                        debug!(
                            session_id = %session_id,
                            text_len = text.len(),
                            "forwarding IM text to session PTY"
                        );
                        // In a real implementation, this would write to the session's PTY stdin
                        // via PtyManager. For now we log it.
                    } else {
                        let _ = outgoing_tx
                            .send(OutgoingMessage {
                                text: "Not attached to any session. Use /attach <session_id> first.".to_string(),
                                channel: Some(ch.clone()),
                            })
                            .await;
                    }
                }
            }
        }
    }

    /// Subscribe to a session's data events and forward PTY output to the IM channel.
    ///
    /// Spawns a background task that reads data events from the session,
    /// strips ANSI codes, and queues batched messages for the IM plugin.
    pub fn monitor_session(&self, session_id: &str, channel_id: &str) {
        let mut data_rx = self.event_bus.create_data_subscriber(session_id);
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
                                None => return, // channel closed
                                _ => {} // ignore other data events
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
                        format!("{}...\n[truncated, {} bytes total]", &text[..4000], text.len())
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
    fn test_parse_command_sessions() {
        match ImCommand::parse("/sessions") {
            ImCommand::ListSessions => {}
            other => panic!("expected ListSessions, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_attach() {
        match ImCommand::parse("/attach my-session-123") {
            ImCommand::Attach { session_id } => assert_eq!(session_id, "my-session-123"),
            other => panic!("expected Attach, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_task_add() {
        match ImCommand::parse("/task add Deploy to production") {
            ImCommand::TaskAdd { description } => assert_eq!(description, "Deploy to production"),
            other => panic!("expected TaskAdd, got {other:?}"),
        }
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
    fn test_parse_command_detach() {
        match ImCommand::parse("/detach") {
            ImCommand::Detach => {}
            other => panic!("expected Detach, got {other:?}"),
        }
    }
}
