//! ACP subprocess client.
//!
//! Spawns an agent binary as a subprocess and communicates via JSON-RPC 2.0
//! over stdin/stdout. Handles initialization handshake, message sending,
//! streaming notifications, and tool use approval/denial.

use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, warn};

use crate::events::{AgentContent, AgentStatus, ErrorClass};

use super::types::*;

/// Default timeout for JSON-RPC calls (30 seconds).
const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// Timeout for the initialization handshake (5 seconds).
const INIT_TIMEOUT_SECS: u64 = 5;
/// Maximum message size (1 MB).
const MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Errors from the ACP client.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    #[error("agent process not running")]
    NotRunning,
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("initialization failed: {0}")]
    InitFailed(String),
    #[error("initialization timed out after {0}s — binary may not support ACP protocol")]
    InitTimeout(u64),
    #[error("timeout waiting for response")]
    Timeout,
    #[error("message too large: {0} bytes")]
    MessageTooLarge(usize),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("RPC error: code={code}, message={message}")]
    RpcError { code: i32, message: String },
    #[error("agent exited with code {0:?}")]
    ProcessExited(Option<i32>),
    #[error("channel closed")]
    ChannelClosed,
}

/// Pending request waiting for a response.
struct PendingRequest {
    tx: oneshot::Sender<Result<JsonRpcResponse, AcpError>>,
}

/// Events emitted by the ACP client to be consumed by the manager.
#[derive(Debug, Clone)]
pub enum AcpEvent {
    /// Agent status changed.
    StatusChanged(AgentStatus),
    /// Agent produced content (streaming or final).
    Content(AgentContent),
    /// Agent requests tool use approval.
    ToolUseRequest {
        id: String,
        tool: String,
        input: serde_json::Value,
    },
    /// Agent encountered an error.
    Error {
        message: String,
        class: ErrorClass,
    },
    /// Agent process exited.
    Exited(Option<i32>),
}

/// ACP client that manages communication with a single agent subprocess.
pub struct AcpClient {
    /// Session ID this client belongs to.
    pub session_id: String,
    /// Agent provider name.
    pub provider: String,
    /// Agent model.
    pub model: String,
    /// Working directory.
    pub cwd: PathBuf,
    /// Current agent status.
    pub status: AgentStatus,
    /// Capabilities discovered during initialization.
    pub capabilities: Option<AcpCapabilities>,

    /// The child process.
    child: Option<Child>,
    /// Stdin writer.
    stdin_tx: Option<Arc<Mutex<tokio::process::ChildStdin>>>,
    /// Pending requests.
    pending: Arc<dashmap::DashMap<String, PendingRequest>>,
    /// Monotonically increasing request ID.
    next_id: AtomicI64,
    /// Channel for events from the agent.
    event_tx: mpsc::Sender<AcpEvent>,
    /// Receiver for events (taken by the manager).
    event_rx: Option<mpsc::Receiver<AcpEvent>>,
    /// Sequence number for outgoing data events.
    seq: std::sync::atomic::AtomicU64,
}

impl AcpClient {
    /// Create a new ACP client (does not start the agent).
    pub fn new(
        session_id: String,
        provider: String,
        model: String,
        cwd: PathBuf,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(512);
        Self {
            session_id,
            provider,
            model,
            cwd,
            status: AgentStatus::Initializing,
            capabilities: None,
            child: None,
            stdin_tx: None,
            pending: Arc::new(dashmap::DashMap::new()),
            next_id: AtomicI64::new(1),
            event_tx,
            event_rx: Some(event_rx),
            seq: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Take the event receiver. Can only be called once.
    pub fn take_event_rx(&mut self) -> Option<mpsc::Receiver<AcpEvent>> {
        self.event_rx.take()
    }

    /// Get the next sequence number.
    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst)
    }

    /// Spawn the agent subprocess and perform initialization.
    pub async fn start(&mut self, agent_binary: &str) -> Result<(), AcpError> {
        debug!(
            session_id = %self.session_id,
            provider = %self.provider,
            binary = %agent_binary,
            "spawning agent process"
        );

        // Check that the binary exists on PATH before attempting to spawn.
        // This gives a clear error instead of a generic "No such file" from the OS.
        if std::process::Command::new("which")
            .arg(agent_binary)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| !s.success())
            .unwrap_or(true)
        {
            return Err(AcpError::SpawnFailed(format!(
                "binary '{}' not found in PATH",
                agent_binary
            )));
        }

        let mut child = Command::new(agent_binary)
            .current_dir(&self.cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AcpError::SpawnFailed(e.to_string()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AcpError::SpawnFailed("failed to capture stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AcpError::SpawnFailed("failed to capture stdout".into()))?;

        let stdin_tx = Arc::new(Mutex::new(stdin));
        self.stdin_tx = Some(Arc::clone(&stdin_tx));
        self.child = Some(child);

        // Start the stdout reader task
        let pending = Arc::clone(&self.pending);
        let event_tx = self.event_tx.clone();
        let session_id = self.session_id.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if line.len() > MAX_MESSAGE_SIZE {
                            warn!(session_id = %session_id, size = line.len(), "message too large");
                            continue;
                        }

                        Self::handle_line(&session_id, &line, &pending, &event_tx).await;
                    }
                    Ok(None) => {
                        debug!(session_id = %session_id, "agent stdout closed");
                        let _ = event_tx.send(AcpEvent::Exited(None)).await;
                        break;
                    }
                    Err(e) => {
                        error!(session_id = %session_id, error = %e, "stdout read error");
                        let _ = event_tx.send(AcpEvent::Error {
                            message: e.to_string(),
                            class: ErrorClass::Transient,
                        }).await;
                        break;
                    }
                }
            }

            // Cancel all pending requests
            let keys: Vec<String> = pending.iter().map(|e| e.key().clone()).collect();
            for key in keys {
                if let Some((_, req)) = pending.remove(&key) {
                    let _ = req.tx.send(Err(AcpError::ProcessExited(None)));
                }
            }
        });

        // Perform initialization handshake with a dedicated timeout.
        // If the binary doesn't speak ACP protocol, this prevents hanging forever.
        match timeout(Duration::from_secs(INIT_TIMEOUT_SECS), self.initialize()).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // Timed out — kill the child so it doesn't linger.
                if let Some(mut child) = self.child.take() {
                    let _ = child.kill().await;
                }
                self.stdin_tx = None;
                Err(AcpError::InitTimeout(INIT_TIMEOUT_SECS))
            }
        }
    }

    /// Handle a single line from the agent's stdout.
    async fn handle_line(
        session_id: &str,
        line: &str,
        pending: &dashmap::DashMap<String, PendingRequest>,
        event_tx: &mpsc::Sender<AcpEvent>,
    ) {
        // Try parsing as a JSON-RPC response first (has "id" + "result"|"error")
        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                warn!(session_id = %session_id, error = %e, "failed to parse JSON");
                return;
            }
        };

        let obj = match value.as_object() {
            Some(o) => o,
            None => return,
        };

        let has_id = obj.contains_key("id");
        let has_method = obj.contains_key("method");
        let has_result = obj.contains_key("result");
        let has_error = obj.contains_key("error");

        if has_id && (has_result || has_error) {
            // This is a response
            if let Ok(resp) = serde_json::from_value::<JsonRpcResponse>(value) {
                let id_str = resp.id.to_string();
                if let Some((_, req)) = pending.remove(&id_str) {
                    let _ = req.tx.send(Ok(resp));
                }
            }
        } else if has_method && !has_id {
            // This is a notification
            if let Ok(notif) = serde_json::from_value::<JsonRpcNotification>(value) {
                Self::handle_notification(session_id, notif, event_tx).await;
            }
        }
    }

    /// Handle an incoming notification from the agent.
    async fn handle_notification(
        session_id: &str,
        notif: JsonRpcNotification,
        event_tx: &mpsc::Sender<AcpEvent>,
    ) {
        match notif.method.as_str() {
            acp_methods::MESSAGES_STREAM => {
                if let Some(params) = notif.params {
                    match serde_json::from_value::<AcpStreamNotification>(params) {
                        Ok(stream) => {
                            match stream.event_type {
                                AcpStreamEventType::Text => {
                                    if let Some(AcpStreamContent::Text { text, streaming }) =
                                        stream.content
                                    {
                                        let _ = event_tx
                                            .send(AcpEvent::Content(AgentContent::Text {
                                                text,
                                                streaming,
                                            }))
                                            .await;
                                    }
                                }
                                AcpStreamEventType::Thinking => {
                                    if let Some(AcpStreamContent::Text { text, .. }) =
                                        stream.content
                                    {
                                        let _ = event_tx
                                            .send(AcpEvent::Content(AgentContent::Thinking {
                                                text,
                                            }))
                                            .await;
                                    }
                                }
                                AcpStreamEventType::ToolUse => {
                                    if let Some(AcpStreamContent::ToolUse { id, tool, input }) =
                                        stream.content
                                    {
                                        let _ = event_tx
                                            .send(AcpEvent::ToolUseRequest {
                                                id: id.clone(),
                                                tool: tool.clone(),
                                                input: input.clone(),
                                            })
                                            .await;
                                        let _ = event_tx
                                            .send(AcpEvent::Content(AgentContent::ToolUse {
                                                id,
                                                tool,
                                                input,
                                            }))
                                            .await;
                                        let _ = event_tx
                                            .send(AcpEvent::StatusChanged(
                                                AgentStatus::WaitingApproval,
                                            ))
                                            .await;
                                    }
                                }
                                AcpStreamEventType::ToolResult => {
                                    if let Some(AcpStreamContent::ToolResult {
                                        id,
                                        output,
                                        is_error,
                                    }) = stream.content
                                    {
                                        let _ = event_tx
                                            .send(AcpEvent::Content(AgentContent::ToolResult {
                                                id,
                                                output,
                                                is_error,
                                            }))
                                            .await;
                                    }
                                }
                                AcpStreamEventType::Done => {
                                    let _ = event_tx
                                        .send(AcpEvent::StatusChanged(AgentStatus::Idle))
                                        .await;
                                }
                                AcpStreamEventType::Error => {
                                    if let Some(AcpStreamContent::Error { message, .. }) =
                                        stream.content
                                    {
                                        let _ = event_tx
                                            .send(AcpEvent::Error {
                                                message,
                                                class: ErrorClass::Transient,
                                            })
                                            .await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                session_id = %session_id,
                                error = %e,
                                "failed to parse messages/stream params"
                            );
                        }
                    }
                }
            }
            acp_methods::STATUS_CHANGED => {
                if let Some(params) = notif.params {
                    if let Ok(status_params) =
                        serde_json::from_value::<AcpStatusChangedParams>(params)
                    {
                        let status = match status_params.status.as_str() {
                            "ready" => AgentStatus::Ready,
                            "working" => AgentStatus::Working,
                            "idle" => AgentStatus::Idle,
                            "waiting_approval" => AgentStatus::WaitingApproval,
                            _ => AgentStatus::Working,
                        };
                        let _ = event_tx.send(AcpEvent::StatusChanged(status)).await;
                    }
                }
            }
            acp_methods::ERROR => {
                if let Some(params) = notif.params {
                    if let Some(obj) = params.as_object() {
                        let message = obj
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error")
                            .to_string();
                        let class = if obj
                            .get("permanent")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            ErrorClass::Permanent
                        } else {
                            ErrorClass::Transient
                        };
                        let _ = event_tx.send(AcpEvent::Error { message, class }).await;
                    }
                }
            }
            other => {
                debug!(
                    session_id = %session_id,
                    method = %other,
                    "unknown agent notification"
                );
            }
        }
    }

    /// Perform the initialization handshake.
    async fn initialize(&mut self) -> Result<(), AcpError> {
        let params = AcpInitializeParams {
            provider: self.provider.clone(),
            model: if self.model.is_empty() {
                None
            } else {
                Some(self.model.clone())
            },
            cwd: Some(self.cwd.to_string_lossy().to_string()),
            protocol_version: "1.0".to_string(),
        };

        let resp = self
            .call(
                acp_methods::INITIALIZE,
                Some(serde_json::to_value(&params).map_err(AcpError::Json)?),
            )
            .await?;

        if let Some(err) = resp.error {
            return Err(AcpError::InitFailed(err.message));
        }

        if let Some(result) = resp.result {
            if let Ok(init_result) = serde_json::from_value::<AcpInitializeResult>(result) {
                self.capabilities = Some(init_result.capabilities);
                debug!(
                    session_id = %self.session_id,
                    agent_name = %init_result.name,
                    agent_version = %init_result.version,
                    "agent initialized"
                );
            }
        }

        self.status = AgentStatus::Ready;
        let _ = self
            .event_tx
            .send(AcpEvent::StatusChanged(AgentStatus::Ready))
            .await;

        Ok(())
    }

    /// Send a JSON-RPC request and wait for a response.
    async fn call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, AcpError> {
        let stdin_tx = self.stdin_tx.as_ref().ok_or(AcpError::NotRunning)?;

        let id = RequestId::Number(self.next_id.fetch_add(1, Ordering::SeqCst));
        let request = JsonRpcRequest::new(id.clone(), method, params);
        let json = serde_json::to_string(&request)?;

        if json.len() > MAX_MESSAGE_SIZE {
            return Err(AcpError::MessageTooLarge(json.len()));
        }

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id.to_string(), PendingRequest { tx });

        {
            let mut stdin = stdin_tx.lock().await;
            stdin.write_all(json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        match timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.pending.remove(&id.to_string());
                Err(AcpError::ProcessExited(None))
            }
            Err(_) => {
                self.pending.remove(&id.to_string());
                Err(AcpError::Timeout)
            }
        }
    }

    /// Send a message to the agent.
    pub async fn send_message(&self, text: String) -> Result<(), AcpError> {
        let _seq = self.next_seq();
        let params = AcpMessagesCreateParams {
            text,
            context: None,
        };

        let resp = self
            .call(
                acp_methods::MESSAGES_CREATE,
                Some(serde_json::to_value(&params).map_err(AcpError::Json)?),
            )
            .await?;

        if let Some(err) = resp.error {
            return Err(AcpError::RpcError {
                code: err.code,
                message: err.message,
            });
        }

        let _ = self
            .event_tx
            .send(AcpEvent::StatusChanged(AgentStatus::Working))
            .await;

        Ok(())
    }

    /// Approve a pending tool use request.
    pub async fn approve_tool(&self, tool_id: String) -> Result<(), AcpError> {
        let params = AcpToolApproveParams { tool_id };
        let resp = self
            .call(
                acp_methods::TOOL_APPROVE,
                Some(serde_json::to_value(&params).map_err(AcpError::Json)?),
            )
            .await?;

        if let Some(err) = resp.error {
            return Err(AcpError::RpcError {
                code: err.code,
                message: err.message,
            });
        }

        let _ = self
            .event_tx
            .send(AcpEvent::StatusChanged(AgentStatus::Working))
            .await;

        Ok(())
    }

    /// Deny a pending tool use request.
    pub async fn deny_tool(&self, tool_id: String, reason: Option<String>) -> Result<(), AcpError> {
        let params = AcpToolDenyParams { tool_id, reason };
        let resp = self
            .call(
                acp_methods::TOOL_DENY,
                Some(serde_json::to_value(&params).map_err(AcpError::Json)?),
            )
            .await?;

        if let Some(err) = resp.error {
            return Err(AcpError::RpcError {
                code: err.code,
                message: err.message,
            });
        }

        Ok(())
    }

    /// Kill the agent subprocess.
    pub async fn kill(&mut self) {
        // Try graceful shutdown first
        if self.stdin_tx.is_some() {
            let _ = self
                .call(acp_methods::SHUTDOWN, None)
                .await;
        }

        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            debug!(session_id = %self.session_id, "killed agent process");
        }
        self.stdin_tx = None;
        self.status = AgentStatus::Crashed {
            error: "killed".to_string(),
            class: ErrorClass::Permanent,
        };
    }

    /// Check if the agent process is still running.
    pub fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }
}
