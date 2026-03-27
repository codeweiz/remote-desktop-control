//! Unified ACP backend for all agent types.
//!
//! `AcpBackend` is the single entry-point used by the rest of RTB to drive any
//! supported agent (Claude, Gemini, OpenCode, Codex).  It:
//!
//! 1. Spawns a dedicated thread with its own single-threaded tokio runtime +
//!    `LocalSet` (ACP futures are `!Send`).
//! 2. Creates the ACP connection (native subprocess or Claude bridge).
//! 3. Runs the Initialize -> NewSession handshake.
//! 4. Enters a command loop receiving `AcpCmd` messages.
//! 5. Broadcasts `AgentEvent` to all subscribers.

use std::path::{Path, PathBuf};

use tokio::sync::{broadcast, mpsc, oneshot};

use super::event::{AgentEvent, AgentKind};

// ---------------------------------------------------------------------------
// Internal command type
// ---------------------------------------------------------------------------

/// Commands sent from the public API to the ACP thread.
enum AcpCmd {
    Prompt {
        text: String,
        done_tx: oneshot::Sender<Result<(), String>>,
    },
    Shutdown,
}

// ---------------------------------------------------------------------------
// AcpBackend
// ---------------------------------------------------------------------------

/// Unified agent backend.
///
/// Works for every `AgentKind` — routes Claude through the in-process bridge
/// and all other agents through native ACP subprocesses.
pub struct AcpBackend {
    kind: AgentKind,
    event_tx: broadcast::Sender<AgentEvent>,
    cmd_tx: Option<mpsc::Sender<AcpCmd>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl AcpBackend {
    /// Create a new backend.  No thread is spawned until [`start`] is called.
    pub fn new(kind: AgentKind) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            kind,
            event_tx,
            cmd_tx: None,
            thread_handle: None,
        }
    }

    /// Spawn the dedicated ACP thread, perform Initialize + NewSession, and
    /// return once the handshake succeeds (or an error is reported).
    pub async fn start(&mut self, cwd: &Path, system_prompt: Option<&str>) -> Result<(), String> {
        let cwd = cwd.to_path_buf();
        let event_tx = self.event_tx.clone();
        let kind = self.kind.clone();
        let system_prompt_owned = system_prompt.map(|s| s.to_string());
        let (cmd_tx, cmd_rx) = mpsc::channel::<AcpCmd>(32);
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();

        let handle = std::thread::Builder::new()
            .name(format!("{}-acp", self.kind))
            .spawn(move || {
                run_acp_thread(kind, cwd, event_tx, cmd_rx, ready_tx, system_prompt_owned);
            })
            .map_err(|e| format!("Failed to spawn ACP thread: {}", e))?;

        self.cmd_tx = Some(cmd_tx);
        self.thread_handle = Some(handle);

        match tokio::time::timeout(std::time::Duration::from_secs(15), ready_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("ACP thread died during init".to_string()),
            Err(_) => Err("ACP initialization timed out after 15s".to_string()),
        }
    }

    /// Send a prompt and wait until the agent finishes the turn.
    pub async fn send_message(&self, text: &str) -> Result<(), String> {
        let cmd_tx = self.cmd_tx.as_ref().ok_or("Agent not started")?;
        let (done_tx, done_rx) = oneshot::channel();
        cmd_tx
            .send(AcpCmd::Prompt {
                text: text.to_string(),
                done_tx,
            })
            .await
            .map_err(|_| "ACP thread gone".to_string())?;
        done_rx.await.map_err(|_| "ACP thread gone".to_string())?
    }

    /// Send a prompt without waiting for completion.  The caller uses the
    /// event stream to detect when the turn finishes.
    pub async fn send_message_fire(&self, text: &str) -> Result<(), String> {
        let cmd_tx = self.cmd_tx.as_ref().ok_or("Agent not started")?;
        let (done_tx, _done_rx) = oneshot::channel();
        cmd_tx
            .send(AcpCmd::Prompt {
                text: text.to_string(),
                done_tx,
            })
            .await
            .map_err(|_| "ACP thread gone".to_string())?;
        Ok(())
    }

    /// Subscribe to the `AgentEvent` broadcast stream.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_tx.subscribe()
    }

    /// Gracefully shut down the ACP thread.
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(AcpCmd::Shutdown).await;
        }
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
        tracing::info!("[{}-acp] shutdown", self.kind);
    }

    /// The agent kind this backend is configured for.
    pub fn kind(&self) -> &AgentKind {
        &self.kind
    }
}

// ---------------------------------------------------------------------------
// Dedicated thread entry-point
// ---------------------------------------------------------------------------

/// Runs on a dedicated thread with a single-threaded tokio runtime + LocalSet.
fn run_acp_thread(
    kind: AgentKind,
    cwd: PathBuf,
    event_tx: broadcast::Sender<AgentEvent>,
    cmd_rx: mpsc::Receiver<AcpCmd>,
    ready_tx: oneshot::Sender<Result<(), String>>,
    system_prompt: Option<String>,
) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("Failed to build runtime: {}", e)));
            return;
        }
    };

    let kind_name = kind.to_string();
    rt.block_on(async move {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                match acp_session_loop(kind, cwd, event_tx, cmd_rx, ready_tx, system_prompt).await {
                    Ok(()) => {}
                    Err(e) => tracing::error!("[{}-acp] session loop error: {}", kind_name, e),
                }
            })
            .await;
    });
}

// ---------------------------------------------------------------------------
// ACP session lifecycle
// ---------------------------------------------------------------------------

/// The actual ACP session lifecycle, running inside LocalSet.
/// Handles both Claude (via in-process duplex pipe) and native ACP agents.
async fn acp_session_loop(
    kind: AgentKind,
    cwd: PathBuf,
    event_tx: broadcast::Sender<AgentEvent>,
    mut cmd_rx: mpsc::Receiver<AcpCmd>,
    ready_tx: oneshot::Sender<Result<(), String>>,
    system_prompt: Option<String>,
) -> Result<(), String> {
    use acp::Agent as _;
    use agent_client_protocol as acp;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    // --- Obtain read/write streams depending on agent kind -------------------
    tracing::info!("[{}-acp] step: obtaining read/write streams", kind);
    let (read_stream, write_stream, _claude_thread): (
        tokio::io::DuplexStream,
        tokio::io::DuplexStream,
        Option<std::thread::JoinHandle<()>>,
    ) = match kind {
        AgentKind::Claude => {
            tracing::info!("[{}-acp] step: spawning claude bridge", kind);
            let (r, w, h) =
                super::claude_bridge::spawn_claude_bridge(cwd.clone(), system_prompt.clone());
            tracing::info!("[{}-acp] step: claude bridge spawned", kind);
            (r, w, Some(h))
        }
        _ => {
            tracing::info!("[{}-acp] step: spawning native ACP subprocess", kind);
            let (r, w) =
                super::native_acp::spawn_native_acp(&kind, &cwd, system_prompt.as_deref())?;
            tracing::info!("[{}-acp] step: native ACP subprocess spawned", kind);
            (r, w, None)
        }
    };

    // --- Create ACP ClientSideConnection ------------------------------------
    tracing::info!("[{}-acp] step: creating ClientSideConnection", kind);
    let client_handler = SharedAcpClientHandler {
        event_tx: event_tx.clone(),
    };
    let (conn, handle_io) = acp::ClientSideConnection::new(
        client_handler,
        write_stream.compat_write(),
        read_stream.compat(),
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );
    tokio::task::spawn_local(handle_io);
    tracing::info!(
        "[{}-acp] step: ClientSideConnection created, IO task spawned",
        kind
    );

    // --- Initialize ----------------------------------------------------------
    tracing::info!("[{}-acp] step: sending initialize request...", kind);
    let _init_resp = conn
        .initialize(
            acp::InitializeRequest::new(acp::ProtocolVersion::V1)
                .client_info(acp::Implementation::new("rtb", "2.0.0").title("RTB")),
        )
        .await
        .map_err(|e| format!("ACP initialize failed: {}", e))?;
    tracing::info!("[{}-acp] step: initialize ok", kind);

    // --- Create session ------------------------------------------------------
    tracing::info!("[{}-acp] step: creating session in {:?}...", kind, &cwd);
    let session_resp = conn
        .new_session(acp::NewSessionRequest::new(cwd))
        .await
        .map_err(|e| format!("ACP new_session failed: {}", e))?;

    let session_id = session_resp.session_id;
    tracing::info!("[{}-acp] step: session created: {:?}", kind, session_id);

    // Signal that initialization is complete.
    let _ = ready_tx.send(Ok(()));

    // --- Command loop --------------------------------------------------------
    loop {
        let cmd = match cmd_rx.recv().await {
            Some(c) => c,
            None => break,
        };
        match cmd {
            AcpCmd::Prompt { text, done_tx } => {
                tracing::debug!("[{}-acp] sending prompt: {}", kind, &text);
                let text_content = acp::ContentBlock::Text(acp::TextContent::new(&text));
                let result = conn
                    .prompt(acp::PromptRequest::new(
                        session_id.clone(),
                        vec![text_content],
                    ))
                    .await;
                tracing::debug!("[{}-acp] prompt returned: {:?}", kind, result.is_ok());
                match result {
                    Ok(_) => {
                        let _ = event_tx.send(AgentEvent::TurnComplete {
                            session_id: Some(session_id.to_string()),
                            cost_usd: None,
                        });
                        let _ = done_tx.send(Ok(()));
                    }
                    Err(e) => {
                        let err = format!("ACP prompt error: {}", e);
                        let _ = event_tx.send(AgentEvent::Error(err.clone()));
                        let _ = done_tx.send(Err(err));
                    }
                }
            }
            AcpCmd::Shutdown => break,
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// SharedAcpClientHandler -- translates ACP notifications into AgentEvents
// ---------------------------------------------------------------------------

/// Implements the ACP `Client` trait -- receives notifications from any ACP
/// agent and broadcasts them as `AgentEvent`s.
struct SharedAcpClientHandler {
    event_tx: broadcast::Sender<AgentEvent>,
}

#[async_trait::async_trait(?Send)]
impl agent_client_protocol::Client for SharedAcpClientHandler {
    async fn request_permission(
        &self,
        args: agent_client_protocol::RequestPermissionRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::RequestPermissionResponse> {
        // Auto-approve: pick the first offered option.
        let option_id = args
            .options
            .first()
            .map(|o| o.option_id.clone())
            .unwrap_or_else(|| "allow".into());
        Ok(agent_client_protocol::RequestPermissionResponse::new(
            agent_client_protocol::RequestPermissionOutcome::Selected(
                agent_client_protocol::SelectedPermissionOutcome::new(option_id),
            ),
        ))
    }

    async fn session_notification(
        &self,
        args: agent_client_protocol::SessionNotification,
    ) -> agent_client_protocol::Result<()> {
        use agent_client_protocol::{ContentBlock, SessionUpdate};

        match args.update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(t) = chunk.content {
                    let _ = self.event_tx.send(AgentEvent::Text(t.text));
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                if let ContentBlock::Text(t) = chunk.content {
                    let _ = self.event_tx.send(AgentEvent::Thinking(t.text));
                }
            }
            SessionUpdate::ToolCallUpdate(update) => {
                let name = update
                    .fields
                    .title
                    .clone()
                    .unwrap_or_else(|| "unknown".into());
                let id = update.tool_call_id.to_string();

                let has_output = update.fields.raw_output.is_some();
                let status_completed = update
                    .fields
                    .status
                    .as_ref()
                    .map(|s| {
                        matches!(
                            s,
                            agent_client_protocol::ToolCallStatus::Completed
                                | agent_client_protocol::ToolCallStatus::Failed
                        )
                    })
                    .unwrap_or(false);

                if has_output || status_completed {
                    // Tool result
                    let output = update
                        .fields
                        .raw_output
                        .as_ref()
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                v.to_string()
                            }
                        })
                        .or_else(|| {
                            update.fields.content.as_ref().map(|blocks| {
                                blocks
                                    .iter()
                                    .filter_map(|block| match block {
                                        agent_client_protocol::ToolCallContent::Content(c) => {
                                            if let ContentBlock::Text(t) = &c.content {
                                                Some(t.text.clone())
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("")
                            })
                        });
                    let is_error = matches!(
                        update.fields.status.as_ref(),
                        Some(agent_client_protocol::ToolCallStatus::Failed)
                    );
                    let _ = self.event_tx.send(AgentEvent::ToolResult {
                        id,
                        output,
                        is_error,
                    });
                } else {
                    // Tool use start / progress
                    let input = update.fields.raw_input.as_ref().map(|v| {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            v.to_string()
                        }
                    });
                    let _ = self.event_tx.send(AgentEvent::ToolUse { name, id, input });
                }
            }
            _ => {}
        }
        Ok(())
    }
}
