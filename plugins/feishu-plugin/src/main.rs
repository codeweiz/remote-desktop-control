//! Feishu (Lark) IM Plugin for RTB
//!
//! A standalone binary that communicates with the RTB host via JSON-RPC 2.0
//! over stdin/stdout (newline-delimited JSON). Implements real Feishu Open API
//! calls for authentication and message sending.
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
use std::time::Instant;

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
// Feishu API base URL
// ---------------------------------------------------------------------------

const FEISHU_API_BASE: &str = "https://open.feishu.cn/open-apis";

// ---------------------------------------------------------------------------
// Feishu API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TenantAccessTokenResponse {
    code: i64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    tenant_access_token: String,
    #[serde(default)]
    expire: u64,
}

#[derive(Debug, Deserialize)]
struct FeishuApiResponse {
    code: i64,
    #[serde(default)]
    msg: String,
}

// ---------------------------------------------------------------------------
// Feishu client with real API calls
// ---------------------------------------------------------------------------

/// Feishu client that uses the Feishu Open API for authentication and messaging.
struct FeishuClient {
    app_id: String,
    app_secret: String,
    /// Default chat_id from config (can be overridden per-message).
    default_chat_id: Option<String>,
    /// Cached tenant access token.
    access_token: Option<String>,
    /// When the current token expires.
    token_expires_at: Option<Instant>,
    /// HTTP client for Feishu API calls.
    http: reqwest::Client,
    /// Whether we are in dry-run mode (no credentials).
    dry_run: bool,
}

impl FeishuClient {
    fn new(app_id: String, app_secret: String, default_chat_id: Option<String>) -> Self {
        let dry_run = app_id.is_empty() || app_secret.is_empty();
        Self {
            app_id,
            app_secret,
            default_chat_id,
            access_token: None,
            token_expires_at: None,
            http: reqwest::Client::new(),
            dry_run,
        }
    }

    /// Obtain a tenant_access_token from Feishu.
    async fn fetch_tenant_token(&self) -> Result<(String, u64), String> {
        let url = format!("{FEISHU_API_BASE}/auth/v3/tenant_access_token/internal/");
        let body = serde_json::json!({
            "app_id": self.app_id,
            "app_secret": self.app_secret,
        });

        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("read body failed: {e}"))?;

        if !status.is_success() {
            return Err(format!("HTTP {status}: {text}"));
        }

        let data: TenantAccessTokenResponse =
            serde_json::from_str(&text).map_err(|e| format!("parse response failed: {e}"))?;

        if data.code != 0 {
            return Err(format!("Feishu auth error (code={}): {}", data.code, data.msg));
        }

        if data.tenant_access_token.is_empty() {
            return Err("Feishu returned empty tenant_access_token".to_string());
        }

        Ok((data.tenant_access_token, data.expire))
    }

    /// Ensure we have a valid (non-expired) access token. Refreshes if needed.
    async fn ensure_token(&mut self) -> Result<String, String> {
        if self.dry_run {
            return Err("dry-run mode: no credentials configured".to_string());
        }

        // Check if current token is still valid (with 60s safety margin).
        if let (Some(token), Some(expires_at)) = (&self.access_token, &self.token_expires_at) {
            if Instant::now() + std::time::Duration::from_secs(60) < *expires_at {
                return Ok(token.clone());
            }
            info!("feishu: access token expired or expiring soon, refreshing");
        }

        let (token, expire_secs) = self.fetch_tenant_token().await?;
        info!(expire_secs, "feishu: obtained new tenant_access_token");
        self.token_expires_at = Some(Instant::now() + std::time::Duration::from_secs(expire_secs));
        self.access_token = Some(token.clone());
        Ok(token)
    }

    /// Connect to Feishu by obtaining a tenant_access_token.
    async fn connect(&mut self) -> Result<(), String> {
        if self.dry_run {
            info!("feishu: no app_id/app_secret configured, running in dry-run mode");
            return Ok(());
        }

        info!(app_id = %self.app_id, "feishu: authenticating with Feishu Open API");
        self.ensure_token().await?;
        info!("feishu: authentication successful");
        Ok(())
    }

    /// Resolve the chat_id: prefer per-message override, then default from config.
    /// Returns an owned String to avoid borrow conflicts with `&mut self` methods.
    fn resolve_chat_id(&self, override_id: Option<&str>) -> Result<String, String> {
        override_id
            .map(|s| s.to_string())
            .or_else(|| self.default_chat_id.clone())
            .ok_or_else(|| "no chat_id: provide 'channel' in params or 'chat_id' in config".to_string())
    }

    /// Send a text message to a Feishu chat via the IM v1 API.
    async fn send_message(
        &mut self,
        text: &str,
        chat_id_override: Option<&str>,
    ) -> Result<(), String> {
        let chat_id = self.resolve_chat_id(chat_id_override)?;

        if self.dry_run {
            info!(text_len = text.len(), chat_id, "feishu: send_message (dry-run)");
            return Ok(());
        }

        // Ensure we have a valid token (auto-refresh if expired).
        let token = self.ensure_token().await?;

        let url = format!("{FEISHU_API_BASE}/im/v1/messages?receive_id_type=chat_id");
        let content = serde_json::json!({"text": text}).to_string();
        let body = serde_json::json!({
            "receive_id": chat_id,
            "msg_type": "text",
            "content": content,
        });

        info!(text_len = text.len(), chat_id, "feishu: sending message");
        debug!(text = %text, "feishu: message content");

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let status = resp.status();
        let text_body = resp.text().await.map_err(|e| format!("read body failed: {e}"))?;

        if !status.is_success() {
            return Err(format!("HTTP {status}: {text_body}"));
        }

        let data: FeishuApiResponse =
            serde_json::from_str(&text_body).map_err(|e| format!("parse response failed: {e}"))?;

        if data.code != 0 {
            // Token may have been revoked server-side; attempt one retry.
            if data.code == 99991663 || data.code == 99991664 {
                warn!("feishu: token invalid/expired (code={}), retrying with fresh token", data.code);
                self.access_token = None;
                self.token_expires_at = None;
                let new_token = self.ensure_token().await?;

                let retry_resp = self
                    .http
                    .post(&url)
                    .header("Authorization", format!("Bearer {new_token}"))
                    .header("Content-Type", "application/json; charset=utf-8")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("HTTP retry request failed: {e}"))?;

                let retry_status = retry_resp.status();
                let retry_body = retry_resp.text().await.map_err(|e| format!("read retry body failed: {e}"))?;

                if !retry_status.is_success() {
                    return Err(format!("HTTP {retry_status} on retry: {retry_body}"));
                }

                let retry_data: FeishuApiResponse = serde_json::from_str(&retry_body)
                    .map_err(|e| format!("parse retry response failed: {e}"))?;

                if retry_data.code != 0 {
                    return Err(format!(
                        "Feishu send error on retry (code={}): {}",
                        retry_data.code, retry_data.msg
                    ));
                }

                info!("feishu: message sent successfully (after token refresh)");
                return Ok(());
            }

            return Err(format!("Feishu send error (code={}): {}", data.code, data.msg));
        }

        info!("feishu: message sent successfully");
        Ok(())
    }

    /// Send an image to a Feishu chat.
    ///
    /// Steps: upload the image to get an `image_key`, then send an image message.
    async fn send_image(
        &mut self,
        data: &str,
        _mime_type: &str,
        caption: Option<&str>,
        chat_id_override: Option<&str>,
    ) -> Result<(), String> {
        let chat_id = self.resolve_chat_id(chat_id_override)?;

        if self.dry_run {
            info!(caption = ?caption, chat_id, "feishu: send_image (dry-run)");
            return Ok(());
        }

        let token = self.ensure_token().await?;

        // Step 1: Upload image
        let upload_url = format!("{FEISHU_API_BASE}/im/v1/images");
        let image_bytes = base64_decode(data)
            .map_err(|e| format!("invalid base64 image data: {e}"))?;

        let part = reqwest::multipart::Part::bytes(image_bytes)
            .file_name("image.png")
            .mime_str("application/octet-stream")
            .map_err(|e| format!("multipart error: {e}"))?;

        let form = reqwest::multipart::Form::new()
            .text("image_type", "message")
            .part("image", part);

        let upload_resp = self
            .http
            .post(&upload_url)
            .header("Authorization", format!("Bearer {token}"))
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("image upload HTTP failed: {e}"))?;

        let upload_status = upload_resp.status();
        let upload_body = upload_resp.text().await.map_err(|e| format!("read upload body failed: {e}"))?;

        if !upload_status.is_success() {
            return Err(format!("image upload HTTP {upload_status}: {upload_body}"));
        }

        let upload_data: serde_json::Value =
            serde_json::from_str(&upload_body).map_err(|e| format!("parse upload response: {e}"))?;

        let code = upload_data.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = upload_data.get("msg").and_then(|v| v.as_str()).unwrap_or("unknown");
            return Err(format!("image upload error (code={code}): {msg}"));
        }

        let image_key = upload_data
            .get("data")
            .and_then(|d| d.get("image_key"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "no image_key in upload response".to_string())?;

        info!(image_key, "feishu: image uploaded");

        // Step 2: Send image message
        let msg_url = format!("{FEISHU_API_BASE}/im/v1/messages?receive_id_type=chat_id");
        let content = serde_json::json!({"image_key": image_key}).to_string();
        let body = serde_json::json!({
            "receive_id": chat_id,
            "msg_type": "image",
            "content": content,
        });

        let send_resp = self
            .http
            .post(&msg_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("image message HTTP failed: {e}"))?;

        let send_status = send_resp.status();
        let send_body = send_resp.text().await.map_err(|e| format!("read send body failed: {e}"))?;

        if !send_status.is_success() {
            return Err(format!("image message HTTP {send_status}: {send_body}"));
        }

        let send_data: FeishuApiResponse =
            serde_json::from_str(&send_body).map_err(|e| format!("parse send response: {e}"))?;

        if send_data.code != 0 {
            return Err(format!("image message error (code={}): {}", send_data.code, send_data.msg));
        }

        // If there is a caption, send it as a follow-up text message.
        if let Some(cap) = caption {
            if !cap.is_empty() {
                info!("feishu: sending image caption as text");
                // Need to reborrow self mutably through a separate call pattern.
                let cap_content = serde_json::json!({"text": cap}).to_string();
                let cap_body = serde_json::json!({
                    "receive_id": chat_id,
                    "msg_type": "text",
                    "content": cap_content,
                });

                let cap_resp = self
                    .http
                    .post(&msg_url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json; charset=utf-8")
                    .json(&cap_body)
                    .send()
                    .await
                    .map_err(|e| format!("caption send failed: {e}"))?;

                let cap_status = cap_resp.status();
                if !cap_status.is_success() {
                    warn!("feishu: caption send returned HTTP {cap_status} (image was sent)");
                }
            }
        }

        info!("feishu: image sent successfully");
        Ok(())
    }

    /// Start polling for incoming messages (stub).
    ///
    /// Full receiving requires a webhook server or Feishu WebSocket subscription,
    /// which will be implemented in a future iteration. For now this keeps the
    /// background task alive.
    async fn start_message_listener(
        &self,
        tx: mpsc::Sender<JsonRpcNotification>,
    ) {
        info!("feishu: message listener started (receiving not yet implemented)");
        let _tx = tx;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        }
    }
}

/// Simple base64 decoder (standard alphabet, with optional padding).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Use a minimal inline decoder to avoid adding another dependency.
    const TABLE: [u8; 128] = {
        let mut t = [255u8; 128];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut d = 0u8;
        while d < 10 {
            t[(b'0' + d) as usize] = d + 52;
            d += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };

    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=' && b != b'\n' && b != b'\r').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let chunks = bytes.chunks(4);
    for chunk in chunks {
        let mut buf = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            if b >= 128 || TABLE[b as usize] == 255 {
                return Err(format!("invalid base64 byte: {b}"));
            }
            buf[i] = TABLE[b as usize];
        }
        let n = chunk.len();
        if n >= 2 {
            out.push((buf[0] << 2) | (buf[1] >> 4));
        }
        if n >= 3 {
            out.push((buf[1] << 4) | (buf[2] >> 2));
        }
        if n >= 4 {
            out.push((buf[2] << 6) | buf[3]);
        }
    }
    Ok(out)
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

    let chat_id = params
        .config
        .get("chat_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| std::env::var("FEISHU_CHAT_ID").ok());

    // Initialize the Feishu client
    {
        let mut feishu = client.lock().await;
        *feishu = FeishuClient::new(app_id, app_secret, chat_id);

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

    let mut feishu = client.lock().await;
    match feishu
        .send_message(&params.text, params.channel.as_deref())
        .await
    {
        Ok(()) => JsonRpcResponse::success(req.id, serde_json::json!({"ok": true})),
        Err(e) => {
            error!(error = %e, "feishu: send_message failed");
            JsonRpcResponse::error(req.id, -32603, format!("send failed: {e}"))
        }
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

    let mut feishu = client.lock().await;
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
        Err(e) => {
            error!(error = %e, "feishu: send_image failed");
            JsonRpcResponse::error(req.id, -32603, format!("send image failed: {e}"))
        }
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
        None,
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
