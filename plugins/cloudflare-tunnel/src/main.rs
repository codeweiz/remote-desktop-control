//! Cloudflare Tunnel Plugin for RTB
//!
//! A standalone binary that communicates with the RTB host via JSON-RPC 2.0
//! over stdin/stdout (newline-delimited JSON). Uses `cloudflared tunnel`
//! quick-tunnel mode (no Cloudflare account required) to expose a local port
//! via a `*.trycloudflare.com` URL.
//!
//! ## Protocol
//!
//! - Host -> Plugin (requests):
//!   - `tunnel/initialize` — Validate that `cloudflared` is installed, store config
//!   - `tunnel/start`      — Spawn `cloudflared tunnel --url http://localhost:<port>`
//!   - `tunnel/stop`       — Kill the `cloudflared` subprocess
//!   - `tunnel/health`     — Check whether the subprocess is still running
//!
//! - Plugin -> Host (notifications):
//!   - `tunnel/on_status`  — Tunnel status changed (starting / ready / down / error)

use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types (standalone, no dependency on rtb crates)
// ---------------------------------------------------------------------------

const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum RequestId {
    Number(i64),
    String(String),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Number(n) => write!(f, "{n}"),
            RequestId::String(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
    id: RequestId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: RequestId,
}

impl JsonRpcResponse {
    fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    fn error(id: RequestId, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

// ---------------------------------------------------------------------------
// Tunnel-specific types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InitializeParams {
    #[serde(default)]
    config: serde_json::Value,
    local_port: u16,
    #[serde(default)]
    domain: Option<String>,
}

// ---------------------------------------------------------------------------
// Cloudflared manager
// ---------------------------------------------------------------------------

/// Manages the `cloudflared` subprocess lifecycle.
struct CloudflaredManager {
    /// Path to the `cloudflared` binary.
    binary: String,
    /// Local port to tunnel.
    local_port: u16,
    /// The running child process, if any.
    child: Option<Child>,
    /// The public tunnel URL once discovered.
    tunnel_url: Option<String>,
    /// When the tunnel was started.
    started_at: Option<Instant>,
}

impl CloudflaredManager {
    fn new() -> Self {
        Self {
            binary: "cloudflared".to_string(),
            local_port: 0,
            child: None,
            tunnel_url: None,
            started_at: None,
        }
    }

    /// Configure the manager with initialization parameters.
    fn configure(&mut self, params: &InitializeParams) {
        // Allow overriding the binary path via config
        if let Some(bin) = params.config.get("binary").and_then(|v| v.as_str()) {
            if !bin.is_empty() {
                self.binary = bin.to_string();
            }
        }
        self.local_port = params.local_port;
    }

    /// Check that `cloudflared` is available.
    async fn check_binary(&self) -> Result<String, String> {
        let output = tokio::process::Command::new(&self.binary)
            .arg("--version")
            .output()
            .await
            .map_err(|e| {
                format!(
                    "cloudflared binary '{}' not found or not executable: {}",
                    self.binary, e
                )
            })?;

        if !output.status.success() {
            return Err(format!(
                "cloudflared --version exited with status {}",
                output.status
            ));
        }

        let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // cloudflared may print version to stderr on some versions
        if version_str.is_empty() {
            let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Ok(stderr_str)
        } else {
            Ok(version_str)
        }
    }

    /// Spawn `cloudflared tunnel --url http://localhost:<port>`.
    ///
    /// Returns immediately. The caller should read stderr in background to
    /// discover the tunnel URL.
    fn spawn(&mut self) -> Result<tokio::process::ChildStderr, String> {
        let url = format!("http://localhost:{}", self.local_port);
        info!(binary = %self.binary, url = %url, "spawning cloudflared");

        let mut child = Command::new(&self.binary)
            .arg("tunnel")
            .arg("--url")
            .arg(&url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("failed to spawn cloudflared: {e}"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "failed to capture cloudflared stderr".to_string())?;

        self.child = Some(child);
        self.started_at = Some(Instant::now());

        Ok(stderr)
    }

    /// Check if the child process is still running.
    fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(None) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    /// Get uptime in seconds.
    fn uptime_secs(&self) -> u64 {
        self.started_at
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Kill the child process.
    async fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            info!("cloudflared process killed");
        }
        self.tunnel_url = None;
        self.started_at = None;
    }
}

// ---------------------------------------------------------------------------
// Request handler
// ---------------------------------------------------------------------------

/// Handle a single JSON-RPC request and return a response.
async fn handle_request(
    req: JsonRpcRequest,
    manager: &Arc<tokio::sync::Mutex<CloudflaredManager>>,
    notification_tx: &mpsc::Sender<JsonRpcNotification>,
) -> JsonRpcResponse {
    match req.method.as_str() {
        "tunnel/initialize" => handle_initialize(req, manager).await,
        "tunnel/start" => handle_start(req, manager, notification_tx).await,
        "tunnel/stop" => handle_stop(req, manager, notification_tx).await,
        "tunnel/health" => handle_health(req, manager).await,
        _ => JsonRpcResponse::error(
            req.id,
            -32601,
            format!("method not found: {}", req.method),
        ),
    }
}

async fn handle_initialize(
    req: JsonRpcRequest,
    manager: &Arc<tokio::sync::Mutex<CloudflaredManager>>,
) -> JsonRpcResponse {
    info!("handling tunnel/initialize");

    let params: InitializeParams = match req.params {
        Some(p) => match serde_json::from_value(p) {
            Ok(params) => params,
            Err(e) => {
                return JsonRpcResponse::error(
                    req.id,
                    -32602,
                    format!("invalid params: {e}"),
                );
            }
        },
        None => {
            return JsonRpcResponse::error(req.id, -32602, "missing params");
        }
    };

    let mut mgr = manager.lock().await;
    mgr.configure(&params);

    // Check that cloudflared is installed and reachable
    match mgr.check_binary().await {
        Ok(version) => {
            info!(version = %version, "cloudflared detected");
        }
        Err(e) => {
            return JsonRpcResponse::error(req.id, -32603, e);
        }
    }

    JsonRpcResponse::success(
        req.id,
        serde_json::json!({
            "name": "cloudflare-tunnel",
            "version": "0.1.0",
            "capabilities": {
                "supports_custom_domain": false,
                "supports_tls": true
            }
        }),
    )
}

async fn handle_start(
    req: JsonRpcRequest,
    manager: &Arc<tokio::sync::Mutex<CloudflaredManager>>,
    notification_tx: &mpsc::Sender<JsonRpcNotification>,
) -> JsonRpcResponse {
    info!("handling tunnel/start");

    // Send "starting" notification
    let _ = notification_tx
        .send(JsonRpcNotification::new(
            "tunnel/on_status",
            Some(serde_json::json!({
                "status": "starting",
            })),
        ))
        .await;

    // Spawn cloudflared
    let stderr = {
        let mut mgr = manager.lock().await;

        // If already running, stop first
        if mgr.is_running() {
            warn!("cloudflared already running, stopping first");
            mgr.kill().await;
        }

        match mgr.spawn() {
            Ok(stderr) => stderr,
            Err(e) => {
                let _ = notification_tx
                    .send(JsonRpcNotification::new(
                        "tunnel/on_status",
                        Some(serde_json::json!({
                            "status": "error",
                            "reason": e,
                        })),
                    ))
                    .await;
                return JsonRpcResponse::error(req.id, -32603, e);
            }
        }
    };

    // Spawn a background task to read stderr and extract the tunnel URL.
    // cloudflared logs to stderr; the URL line looks like:
    //   "... https://xxx-yyy-zzz.trycloudflare.com ..."
    let manager_clone = Arc::clone(manager);
    let notif_tx = notification_tx.clone();

    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            debug!(line = %line, "cloudflared stderr");

            // Look for the trycloudflare.com URL
            if let Some(url) = extract_tunnel_url(&line) {
                info!(url = %url, "tunnel URL discovered");

                {
                    let mut mgr = manager_clone.lock().await;
                    mgr.tunnel_url = Some(url.clone());
                }

                let _ = notif_tx
                    .send(JsonRpcNotification::new(
                        "tunnel/on_status",
                        Some(serde_json::json!({
                            "status": "ready",
                            "url": url,
                        })),
                    ))
                    .await;
            }
        }

        // stderr closed means cloudflared exited
        warn!("cloudflared stderr closed — process likely exited");

        let _ = notif_tx
            .send(JsonRpcNotification::new(
                "tunnel/on_status",
                Some(serde_json::json!({
                    "status": "down",
                    "reason": "cloudflared process exited",
                })),
            ))
            .await;
    });

    // Return immediately; the tunnel URL will arrive via on_status notification
    JsonRpcResponse::success(
        req.id,
        serde_json::json!({
            "url": "",
            "expires_at": null,
        }),
    )
}

async fn handle_stop(
    req: JsonRpcRequest,
    manager: &Arc<tokio::sync::Mutex<CloudflaredManager>>,
    notification_tx: &mpsc::Sender<JsonRpcNotification>,
) -> JsonRpcResponse {
    info!("handling tunnel/stop");

    let mut mgr = manager.lock().await;
    mgr.kill().await;

    let _ = notification_tx
        .send(JsonRpcNotification::new(
            "tunnel/on_status",
            Some(serde_json::json!({
                "status": "down",
                "reason": "stopped by host",
            })),
        ))
        .await;

    JsonRpcResponse::success(req.id, serde_json::json!({"ok": true}))
}

async fn handle_health(
    req: JsonRpcRequest,
    manager: &Arc<tokio::sync::Mutex<CloudflaredManager>>,
) -> JsonRpcResponse {
    let mut mgr = manager.lock().await;
    let running = mgr.is_running();
    let url = mgr.tunnel_url.clone();
    let uptime = mgr.uptime_secs();

    JsonRpcResponse::success(
        req.id,
        serde_json::json!({
            "healthy": running,
            "url": url,
            "uptime_secs": uptime,
        }),
    )
}

// ---------------------------------------------------------------------------
// URL extraction
// ---------------------------------------------------------------------------

/// Extract a `https://*.trycloudflare.com` URL from a cloudflared log line.
///
/// cloudflared prints a line like:
///   "... https://foo-bar-baz.trycloudflare.com ..."
/// when the quick tunnel is ready.
fn extract_tunnel_url(line: &str) -> Option<String> {
    // Find any https URL containing trycloudflare.com
    for token in line.split_whitespace() {
        if token.starts_with("https://") && token.contains(".trycloudflare.com") {
            // Strip any trailing punctuation that might be part of the log format
            let url = token.trim_end_matches(|c: char| !c.is_alphanumeric());
            return Some(url.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Main loop: read JSON-RPC from stdin, write responses/notifications to stdout
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Initialize tracing to stderr (stdout is reserved for JSON-RPC)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cloudflare_tunnel=debug".parse().unwrap()),
        )
        .with_writer(io::stderr)
        .with_target(false)
        .init();

    info!("cloudflare-tunnel plugin starting");

    let manager = Arc::new(tokio::sync::Mutex::new(CloudflaredManager::new()));

    // Channel for outgoing notifications (plugin -> host)
    let (notification_tx, mut notification_rx) = mpsc::channel::<JsonRpcNotification>(64);

    // Spawn notification writer task (writes notifications to stdout)
    let stdout_lock = Arc::new(tokio::sync::Mutex::new(io::stdout()));
    let stdout_for_notifs = Arc::clone(&stdout_lock);

    tokio::spawn(async move {
        while let Some(notif) = notification_rx.recv().await {
            match serde_json::to_string(&notif) {
                Ok(json) => {
                    let mut stdout = stdout_for_notifs.lock().await;
                    if writeln!(stdout, "{json}").is_err() {
                        break;
                    }
                    let _ = stdout.flush();
                }
                Err(e) => {
                    error!(error = %e, "failed to serialize notification");
                }
            }
        }
    });

    // Read JSON-RPC requests from stdin (blocking read in a spawn_blocking context)
    let (req_tx, mut req_rx) = mpsc::channel::<JsonRpcRequest>(32);

    tokio::task::spawn_blocking(move || {
        let stdin = io::stdin();
        let reader = stdin.lock();

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    let value: serde_json::Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!(error = %e, "failed to parse JSON from stdin");
                            continue;
                        }
                    };

                    // Check if it's a request (has id and method)
                    if let Some(obj) = value.as_object() {
                        if obj.contains_key("id") && obj.contains_key("method") {
                            match serde_json::from_value::<JsonRpcRequest>(
                                serde_json::Value::Object(obj.clone()),
                            ) {
                                Ok(req) => {
                                    if req_tx.blocking_send(req).is_err() {
                                        return; // receiver dropped
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "failed to parse JSON-RPC request");
                                }
                            }
                        } else {
                            debug!("ignoring non-request message from host");
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        info!("stdin closed, shutting down");
                    } else {
                        error!(error = %e, "stdin read error");
                    }
                    return;
                }
            }
        }
    });

    // Process requests
    while let Some(req) = req_rx.recv().await {
        debug!(method = %req.method, id = %req.id, "processing request");

        let response = handle_request(req, &manager, &notification_tx).await;

        // Write response to stdout
        match serde_json::to_string(&response) {
            Ok(json) => {
                let mut stdout = stdout_lock.lock().await;
                if writeln!(stdout, "{json}").is_err() {
                    error!("failed to write response to stdout");
                    break;
                }
                let _ = stdout.flush();
            }
            Err(e) => {
                error!(error = %e, "failed to serialize response");
            }
        }
    }

    // Cleanup: kill cloudflared if still running
    let mut mgr = manager.lock().await;
    mgr.kill().await;

    info!("cloudflare-tunnel plugin shutting down");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tunnel_url_standard() {
        let line = "2024-01-15T10:30:00Z INF +-----------------------------------------------------------+";
        assert_eq!(extract_tunnel_url(line), None);

        let line = "2024-01-15T10:30:00Z INF |  Your quick Tunnel has been created! Visit it at:        |";
        assert_eq!(extract_tunnel_url(line), None);

        let line = "2024-01-15T10:30:00Z INF |  https://foo-bar-baz.trycloudflare.com                   |";
        assert_eq!(
            extract_tunnel_url(line),
            Some("https://foo-bar-baz.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_extract_tunnel_url_inline() {
        let line = "INFO connector]  Your quick Tunnel has been created! Visit it at (it may take some time to be reachable): https://abc-def-ghi.trycloudflare.com";
        assert_eq!(
            extract_tunnel_url(line),
            Some("https://abc-def-ghi.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_extract_tunnel_url_no_match() {
        let line = "some random log line without a URL";
        assert_eq!(extract_tunnel_url(line), None);

        let line = "https://example.com is not a tunnel URL";
        assert_eq!(extract_tunnel_url(line), None);
    }
}
