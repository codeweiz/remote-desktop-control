//! Single plugin process wrapper.
//!
//! Manages the lifecycle of a single plugin subprocess, providing
//! JSON-RPC communication over stdin/stdout pipes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, warn};

use crate::protocol::{
    JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RequestId,
};
use crate::types::{PluginManifest, PluginState};

/// Default JSON-RPC timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// Maximum message size in bytes (1 MB).
const MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Errors from plugin process operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginProcessError {
    #[error("plugin process not running")]
    NotRunning,
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("timeout waiting for response (id={0})")]
    Timeout(String),
    #[error("message too large: {size} bytes (max {MAX_MESSAGE_SIZE})")]
    MessageTooLarge { size: usize },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("plugin returned error: code={code}, message={message}")]
    RpcError { code: i32, message: String },
    #[error("plugin process exited with code {0:?}")]
    ProcessExited(Option<i32>),
}

/// Pending request waiting for a response.
struct PendingRequest {
    tx: oneshot::Sender<Result<JsonRpcResponse, PluginProcessError>>,
}

/// A wrapper around a single plugin subprocess.
///
/// Communication happens via JSON-RPC 2.0 over stdin (write) / stdout (read).
/// Each line on stdout is expected to be a complete JSON-RPC message.
pub struct PluginProcess {
    /// The plugin manifest.
    pub manifest: PluginManifest,
    /// Current state.
    pub state: PluginState,
    /// Path to the plugin directory.
    pub plugin_dir: PathBuf,
    /// Timeout for JSON-RPC calls.
    timeout_secs: u64,
    /// Monotonically increasing request ID.
    next_id: AtomicI64,
    /// The child process handle.
    child: Option<Child>,
    /// Sender for writing to stdin. Protected by mutex for ordered writes.
    stdin_tx: Option<Arc<Mutex<tokio::process::ChildStdin>>>,
    /// Pending requests awaiting responses, keyed by request ID string.
    pending: Arc<dashmap::DashMap<String, PendingRequest>>,
    /// Channel for notifications received from the plugin.
    notification_tx: mpsc::Sender<JsonRpcNotification>,
    /// Receiver for notifications (consumed by the bridge).
    notification_rx: Option<mpsc::Receiver<JsonRpcNotification>>,
}

impl PluginProcess {
    /// Create a new plugin process wrapper (does not start the process).
    pub fn new(manifest: PluginManifest, plugin_dir: PathBuf, timeout_secs: Option<u64>) -> Self {
        let (notification_tx, notification_rx) = mpsc::channel(256);
        Self {
            manifest,
            state: PluginState::Pending,
            plugin_dir,
            timeout_secs: timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
            next_id: AtomicI64::new(1),
            child: None,
            stdin_tx: None,
            pending: Arc::new(dashmap::DashMap::new()),
            notification_tx,
            notification_rx: Some(notification_rx),
        }
    }

    /// Take the notification receiver. Can only be called once.
    pub fn take_notification_rx(&mut self) -> Option<mpsc::Receiver<JsonRpcNotification>> {
        self.notification_rx.take()
    }

    /// Spawn the plugin subprocess.
    pub async fn spawn(&mut self) -> Result<(), PluginProcessError> {
        let executable = self.plugin_dir.join(&self.manifest.plugin.executable);

        debug!(
            plugin_id = %self.manifest.plugin.id,
            executable = %executable.display(),
            "spawning plugin process"
        );

        let mut child = Command::new(&executable)
            .current_dir(&self.plugin_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| PluginProcessError::SpawnFailed(e.to_string()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PluginProcessError::SpawnFailed("failed to capture stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PluginProcessError::SpawnFailed("failed to capture stdout".into()))?;

        let stdin_tx = Arc::new(Mutex::new(stdin));
        self.stdin_tx = Some(Arc::clone(&stdin_tx));
        self.child = Some(child);
        self.state = PluginState::Starting;

        // Spawn stdout reader task
        let pending = Arc::clone(&self.pending);
        let notification_tx = self.notification_tx.clone();
        let plugin_id = self.manifest.plugin.id.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if line.len() > MAX_MESSAGE_SIZE {
                            warn!(
                                plugin_id = %plugin_id,
                                size = line.len(),
                                "message too large, dropping"
                            );
                            continue;
                        }

                        match JsonRpcMessage::parse(&line) {
                            Ok(JsonRpcMessage::Response(resp)) => {
                                let id_str = resp.id.to_string();
                                if let Some((_, pending_req)) = pending.remove(&id_str) {
                                    let _ = pending_req.tx.send(Ok(resp));
                                } else {
                                    warn!(
                                        plugin_id = %plugin_id,
                                        id = %id_str,
                                        "received response for unknown request"
                                    );
                                }
                            }
                            Ok(JsonRpcMessage::Notification(notif)) => {
                                if notification_tx.send(notif).await.is_err() {
                                    debug!(
                                        plugin_id = %plugin_id,
                                        "notification channel closed"
                                    );
                                    break;
                                }
                            }
                            Ok(JsonRpcMessage::Request(req)) => {
                                // Plugins shouldn't send requests to the host in this protocol,
                                // but we log it for debugging.
                                warn!(
                                    plugin_id = %plugin_id,
                                    method = %req.method,
                                    "unexpected request from plugin"
                                );
                            }
                            Err(e) => {
                                warn!(
                                    plugin_id = %plugin_id,
                                    error = %e,
                                    line = %line,
                                    "failed to parse JSON-RPC message"
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        debug!(plugin_id = %plugin_id, "plugin stdout closed");
                        break;
                    }
                    Err(e) => {
                        error!(plugin_id = %plugin_id, error = %e, "stdout read error");
                        break;
                    }
                }
            }

            // Cancel all pending requests
            let keys: Vec<String> = pending.iter().map(|e| e.key().clone()).collect();
            for key in keys {
                if let Some((_, req)) = pending.remove(&key) {
                    let _ = req.tx.send(Err(PluginProcessError::ProcessExited(None)));
                }
            }
        });

        Ok(())
    }

    /// Send a JSON-RPC request and wait for a response.
    pub async fn call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, PluginProcessError> {
        let stdin_tx = self
            .stdin_tx
            .as_ref()
            .ok_or(PluginProcessError::NotRunning)?;

        let id = RequestId::Number(self.next_id.fetch_add(1, Ordering::SeqCst));
        let request = JsonRpcRequest::new(id.clone(), method, params);
        let json = serde_json::to_string(&request)?;

        if json.len() > MAX_MESSAGE_SIZE {
            return Err(PluginProcessError::MessageTooLarge { size: json.len() });
        }

        // Register pending request
        let (tx, rx) = oneshot::channel();
        self.pending.insert(id.to_string(), PendingRequest { tx });

        // Write to stdin
        {
            let mut stdin = stdin_tx.lock().await;
            stdin.write_all(json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        debug!(plugin_id = %self.manifest.plugin.id, method = %method, id = %id, "sent request");

        // Wait for response with timeout
        match timeout(Duration::from_secs(self.timeout_secs), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // oneshot sender dropped (reader task died)
                self.pending.remove(&id.to_string());
                Err(PluginProcessError::ProcessExited(None))
            }
            Err(_) => {
                // Timeout
                self.pending.remove(&id.to_string());
                Err(PluginProcessError::Timeout(id.to_string()))
            }
        }
    }

    /// Send a JSON-RPC notification (fire-and-forget, no response expected).
    pub async fn notify(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), PluginProcessError> {
        let stdin_tx = self
            .stdin_tx
            .as_ref()
            .ok_or(PluginProcessError::NotRunning)?;

        let notification = JsonRpcNotification::new(method, params);
        let json = serde_json::to_string(&notification)?;

        if json.len() > MAX_MESSAGE_SIZE {
            return Err(PluginProcessError::MessageTooLarge { size: json.len() });
        }

        let mut stdin = stdin_tx.lock().await;
        stdin.write_all(json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        Ok(())
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(None) => true, // still running
                Ok(Some(_)) => false,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Kill the plugin subprocess.
    pub async fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            debug!(plugin_id = %self.manifest.plugin.id, "killed plugin process");
        }
        self.stdin_tx = None;
        self.state = PluginState::Stopped;
    }

    /// Get the exit status if the process has exited.
    pub async fn try_wait(&mut self) -> Option<i32> {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => Some(status.code().unwrap_or(-1)),
                _ => None,
            }
        } else {
            None
        }
    }
}
