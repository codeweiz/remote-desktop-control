//! Feishu (Lark) IM Plugin for RTB
//!
//! A standalone binary that communicates with the RTB host via JSON-RPC 2.0
//! over stdin/stdout (newline-delimited JSON). This is a skeleton implementation
//! with placeholder Feishu API calls — the JSON-RPC protocol handling is complete.
//!
//! ## Protocol
//!
//! - Host -> Plugin (requests):
//!   - `im/initialize` — Initialize the Feishu connection
//!   - `im/send_message` — Send a text message to a Feishu chat
//!   - `im/send_image` — Send an image to a Feishu chat
//!   - `im/shutdown` — Graceful shutdown
//!
//! - Plugin -> Host (notifications):
//!   - `im/on_message` — Incoming message from Feishu
//!   - `im/on_status` — Connection status changed

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
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
// Feishu-specific types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InitializeParams {
    #[serde(default)]
    config: serde_json::Value,
    #[serde(default)]
    protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SendMessageParams {
    text: String,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    urgent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SendImageParams {
    data: String,
    mime_type: String,
    #[serde(default)]
    caption: Option<String>,
    #[serde(default)]
    channel: Option<String>,
}

// ---------------------------------------------------------------------------
// Feishu client (placeholder)
// ---------------------------------------------------------------------------

/// Placeholder Feishu client. In a real implementation, this would use
/// the Feishu Open API (https://open.feishu.cn) to send/receive messages.
struct FeishuClient {
    app_id: String,
    app_secret: String,
    _access_token: Option<String>,
}

impl FeishuClient {
    fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            _access_token: None,
        }
    }

    /// Connect to Feishu (placeholder — logs instead of making real API calls).
    async fn connect(&mut self) -> Result<(), String> {
        // In a real implementation:
        // 1. POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal/
        //    with app_id and app_secret to get tenant_access_token
        // 2. Store the token for subsequent API calls
        // 3. Set up a WebSocket or long-poll for receiving messages

        if self.app_id.is_empty() || self.app_secret.is_empty() {
            info!(
                "feishu: no app_id/app_secret configured, running in dry-run mode"
            );
        } else {
            info!(
                app_id = %self.app_id,
                "feishu: connecting (placeholder — not calling real API)"
            );
        }

        // Placeholder: always succeed
        self._access_token = Some("placeholder_token".to_string());
        Ok(())
    }

    /// Send a text message to a Feishu chat (placeholder).
    async fn send_message(
        &self,
        text: &str,
        chat_id: Option<&str>,
    ) -> Result<(), String> {
        // In a real implementation:
        // POST https://open.feishu.cn/open-apis/im/v1/messages
        // with receive_id_type=chat_id, receive_id=<chat_id>,
        // msg_type=text, content={"text": "<text>"}

        info!(
            text_len = text.len(),
            chat_id = ?chat_id,
            "feishu: send_message (placeholder)"
        );
        debug!(text = %text, "feishu: message content");
        Ok(())
    }

    /// Send an image to a Feishu chat (placeholder).
    async fn send_image(
        &self,
        _data: &str,
        _mime_type: &str,
        caption: Option<&str>,
        chat_id: Option<&str>,
    ) -> Result<(), String> {
        // In a real implementation:
        // 1. Upload image via POST https://open.feishu.cn/open-apis/im/v1/images
        // 2. Send message with msg_type=image and the image_key from step 1

        info!(
            caption = ?caption,
            chat_id = ?chat_id,
            "feishu: send_image (placeholder)"
        );
        Ok(())
    }

    /// Start polling for incoming messages (placeholder).
    ///
    /// In a real implementation, this would use Feishu's event subscription
    /// (webhook or WebSocket) to receive messages.
    async fn start_message_listener(
        &self,
        tx: mpsc::Sender<JsonRpcNotification>,
    ) {
        info!("feishu: message listener started (placeholder — no real polling)");

        // In a real implementation, this would:
        // 1. Subscribe to im.message.receive_v1 event via webhook/websocket
        // 2. Parse incoming messages
        // 3. Send im/on_message notifications to the host

        // Placeholder: just keep the task alive
        // In production, replace this with actual Feishu event subscription
        let _tx = tx; // keep the sender alive
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Request handler
// ---------------------------------------------------------------------------

/// Handle a single JSON-RPC request and return a response.
async fn handle_request(
    req: JsonRpcRequest,
    client: &Arc<tokio::sync::Mutex<FeishuClient>>,
    notification_tx: &mpsc::Sender<JsonRpcNotification>,
) -> JsonRpcResponse {
    match req.method.as_str() {
        "im/initialize" => handle_initialize(req, client, notification_tx).await,
        "im/send_message" => handle_send_message(req, client).await,
        "im/send_image" => handle_send_image(req, client).await,
        "im/shutdown" => handle_shutdown(req).await,
        _ => JsonRpcResponse::error(
            req.id,
            -32601,
            format!("method not found: {}", req.method),
        ),
    }
}

async fn handle_initialize(
    req: JsonRpcRequest,
    client: &Arc<tokio::sync::Mutex<FeishuClient>>,
    notification_tx: &mpsc::Sender<JsonRpcNotification>,
) -> JsonRpcResponse {
    info!("handling im/initialize");

    // Parse config from params
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
        None => InitializeParams {
            config: serde_json::Value::Null,
            protocol_version: "1.0".to_string(),
        },
    };

    // Extract Feishu credentials from config or environment
    let app_id = params
        .config
        .get("app_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| std::env::var("FEISHU_APP_ID").ok())
        .unwrap_or_default();

    let app_secret = params
        .config
        .get("app_secret")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| std::env::var("FEISHU_APP_SECRET").ok())
        .unwrap_or_default();

    // Initialize the Feishu client
    {
        let mut feishu = client.lock().await;
        *feishu = FeishuClient::new(app_id, app_secret);

        if let Err(e) = feishu.connect().await {
            // Send status notification
            let _ = notification_tx
                .send(JsonRpcNotification::new(
                    "im/on_status",
                    Some(serde_json::json!({
                        "status": "error",
                        "message": e,
                    })),
                ))
                .await;

            return JsonRpcResponse::error(
                req.id,
                -32603,
                format!("failed to connect to Feishu: {e}"),
            );
        }
    }

    // Send connected status notification
    let _ = notification_tx
        .send(JsonRpcNotification::new(
            "im/on_status",
            Some(serde_json::json!({
                "status": "connected",
            })),
        ))
        .await;

    // Start the message listener in background
    let client_clone = Arc::clone(client);
    let notif_tx = notification_tx.clone();
    tokio::spawn(async move {
        let feishu = client_clone.lock().await;
        feishu.start_message_listener(notif_tx).await;
    });

    JsonRpcResponse::success(
        req.id,
        serde_json::json!({
            "name": "feishu-im",
            "version": "0.1.0",
            "capabilities": {
                "supports_images": true,
                "supports_markdown": false,
                "max_message_length": 0
            }
        }),
    )
}

async fn handle_send_message(
    req: JsonRpcRequest,
    client: &Arc<tokio::sync::Mutex<FeishuClient>>,
) -> JsonRpcResponse {
    let params: SendMessageParams = match req.params {
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

    let feishu = client.lock().await;
    match feishu
        .send_message(&params.text, params.channel.as_deref())
        .await
    {
        Ok(()) => JsonRpcResponse::success(req.id, serde_json::json!({"ok": true})),
        Err(e) => JsonRpcResponse::error(req.id, -32603, format!("send failed: {e}")),
    }
}

async fn handle_send_image(
    req: JsonRpcRequest,
    client: &Arc<tokio::sync::Mutex<FeishuClient>>,
) -> JsonRpcResponse {
    let params: SendImageParams = match req.params {
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

    let feishu = client.lock().await;
    match feishu
        .send_image(
            &params.data,
            &params.mime_type,
            params.caption.as_deref(),
            params.channel.as_deref(),
        )
        .await
    {
        Ok(()) => JsonRpcResponse::success(req.id, serde_json::json!({"ok": true})),
        Err(e) => JsonRpcResponse::error(req.id, -32603, format!("send image failed: {e}")),
    }
}

async fn handle_shutdown(req: JsonRpcRequest) -> JsonRpcResponse {
    info!("handling im/shutdown — will exit after response");
    // Schedule exit after sending response
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        std::process::exit(0);
    });
    JsonRpcResponse::success(req.id, serde_json::json!({"ok": true}))
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
                .add_directive("feishu_plugin=debug".parse().unwrap()),
        )
        .with_writer(io::stderr)
        .with_target(false)
        .init();

    info!("feishu-plugin starting");

    let client = Arc::new(tokio::sync::Mutex::new(FeishuClient::new(
        String::new(),
        String::new(),
    )));

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

                    // Parse the incoming message
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

        let response = handle_request(req, &client, &notification_tx).await;

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

    info!("feishu-plugin shutting down");
}
