# Tunnel Port Passthrough & Feishu WebSocket Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the hardcoded tunnel port and implement Feishu WebSocket long connection for receiving messages.

**Architecture:** Two independent changes: (1) thread `config.server.port` through `PluginManager` to the tunnel plugin, and add the missing tunnel URL event handler in Tauri mode; (2) replace the stub `start_message_listener` in the Feishu plugin with a full WebSocket long connection client using Protobuf framing.

**Tech Stack:** Rust, prost (protobuf), tokio-tungstenite (WebSocket), futures-util

**Spec:** `docs/superpowers/specs/2026-03-26-tunnel-port-feishu-websocket-design.md`

---

### Task 1: Add `server_port` to PluginManager

**Files:**
- Modify: `crates/plugin-host/src/manager.rs:51-70` (struct + constructor)

- [ ] **Step 1: Add `server_port` field to `PluginManager` struct**

In `crates/plugin-host/src/manager.rs`, add `server_port: u16` to the struct (after line 59) and update `new()` to accept and store it:

```rust
// In struct PluginManager (line 51):
pub struct PluginManager {
    plugins_dir: PathBuf,
    plugins: Arc<RwLock<HashMap<String, ManagedPlugin>>>,
    event_bus: Arc<EventBus>,
    timeout_secs: u64,
    server_port: u16,  // NEW
}

// In impl PluginManager::new() (line 64):
pub fn new(plugins_dir: PathBuf, event_bus: Arc<EventBus>, timeout_secs: u64, server_port: u16) -> Self {
    Self {
        plugins_dir,
        plugins: Arc::new(RwLock::new(HashMap::new())),
        event_bus,
        timeout_secs,
        server_port,  // NEW
    }
}
```

- [ ] **Step 2: Replace hardcoded 3000 with `self.server_port`**

Two locations in `start_plugin()`:

Line 224-227 (tunnel/initialize params):
```rust
PluginType::Tunnel => serde_json::json!({
    "config": {},
    "local_port": self.server_port
}),
```

Line 262-264 (tunnel/start params):
```rust
let start_params = serde_json::json!({
    "config": {},
    "local_port": self.server_port
});
```

- [ ] **Step 3: Compile check**

Run: `cargo check -p rtb-plugin-host 2>&1 | head -20`
Expected: Compilation errors in call sites (CLI and Tauri) because `new()` now requires 4 args. This is expected and will be fixed in the next tasks.

- [ ] **Step 4: Commit**

```bash
git add crates/plugin-host/src/manager.rs
git commit -m "feat(plugin-host): add server_port param to PluginManager, replace hardcoded 3000"
```

---

### Task 2: Update CLI call site to pass server port

**Files:**
- Modify: `crates/cli/src/commands/start.rs:80-84`

- [ ] **Step 1: Pass `config.server.port` to `PluginManager::new()`**

At line 80-84, change:
```rust
let plugin_manager = Arc::new(PluginManager::new(
    plugins_dir,
    Arc::clone(&core.event_bus),
    config.plugins.jsonrpc_timeout_secs,
    config.server.port,  // NEW
));
```

- [ ] **Step 2: Compile check**

Run: `cargo check -p rtb-cli 2>&1 | head -20`
Expected: Success (or only Tauri errors remaining).

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/commands/start.rs
git commit -m "fix(cli): pass server port to PluginManager"
```

---

### Task 3: Update Tauri call site + add tunnel URL event handler

**Files:**
- Modify: `crates/tauri-app/src/commands.rs:181-185` (call site) and `~232-249` (add event handler)

- [ ] **Step 1: Pass `config.server.port` to `PluginManager::new()`**

At line 181-185, change:
```rust
let plugin_manager = Arc::new(PluginManager::new(
    plugins_dir,
    Arc::clone(&core.event_bus),
    config.plugins.jsonrpc_timeout_secs,
    config.server.port,  // NEW
));
```

- [ ] **Step 2: Add tunnel URL event subscriber**

Add a new background block **after** the server's `tokio::spawn` block (around line 249) and **before** the notification store listener (line 252). This follows the same pattern as the notification store listener. We use `core.event_bus` (the original Arc, still alive at this scope) and a shared `tunnel_url` that was created earlier inside the spawn block.

First, hoist `tunnel_url` out of the server spawn so it can be shared. Before the server `tokio::spawn` (around line 208), add:

```rust
let tunnel_url: Arc<tokio::sync::RwLock<Option<String>>> =
    Arc::new(tokio::sync::RwLock::new(None));
```

Then use `Arc::clone(&tunnel_url)` when constructing `AppState` inside the spawn (replacing the inline `Arc::new(...)` at line 232).

After the server `tokio::spawn` block closes (after line 249), add:

```rust
// Background: track tunnel URL in AppState
{
    let tunnel_url = Arc::clone(&tunnel_url);
    let mut control_rx = core.event_bus.subscribe_control();
    tokio::spawn(async move {
        loop {
            match control_rx.recv().await {
                Ok(event) => match event.as_ref() {
                    rtb_core::events::ControlEvent::TunnelReady { url } => {
                        tracing::info!(url = %url, "tunnel URL stored");
                        *tunnel_url.write().await = Some(url.clone());
                    }
                    rtb_core::events::ControlEvent::TunnelDown { .. } => {
                        *tunnel_url.write().await = None;
                    }
                    _ => {}
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "tunnel URL listener lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
```

- [ ] **Step 3: Compile check**

Run: `cargo check -p rtb-tauri-app 2>&1 | head -20`
Expected: Success. May need `--features custom-protocol` or similar based on Tauri config.

- [ ] **Step 4: Commit**

```bash
git add crates/tauri-app/src/commands.rs
git commit -m "fix(tauri): pass server port to PluginManager, add tunnel URL event handler"
```

---

### Task 4: Add WebSocket + Protobuf dependencies to Feishu plugin

**Files:**
- Modify: `plugins/feishu-plugin/Cargo.toml`

- [ ] **Step 1: Add new dependencies**

Add after the existing `reqwest` line:

```toml
prost = "0.13"
tokio-tungstenite = { version = "0.26", features = ["native-tls"] }
futures-util = "0.3"
rand = "0.8"
```

(`rand` is for reconnection jitter)

- [ ] **Step 2: Verify build**

Run: `cargo check --manifest-path plugins/feishu-plugin/Cargo.toml 2>&1 | head -20`
Expected: Success (no code changes yet, just deps).

- [ ] **Step 3: Commit**

```bash
git add plugins/feishu-plugin/Cargo.toml plugins/feishu-plugin/Cargo.lock
git commit -m "feat(feishu): add prost, tokio-tungstenite, futures-util, rand dependencies"
```

---

### Task 5: Add Protobuf Frame types to Feishu plugin

**Files:**
- Modify: `plugins/feishu-plugin/src/main.rs` (add after the JSON-RPC types section, around line 116)

- [ ] **Step 1: Add Protobuf structs and WebSocket endpoint types**

Insert after the `JsonRpcNotification` impl block (after line 116):

```rust
// ---------------------------------------------------------------------------
// Feishu WebSocket long connection types (Protobuf)
// ---------------------------------------------------------------------------

/// Protobuf Frame — the binary envelope for all Feishu WebSocket messages.
/// Reverse-engineered from official Feishu Go/Python SDKs.
#[derive(Clone, PartialEq, prost::Message)]
pub struct WsFrame {
    #[prost(uint64, required, tag = "1")]
    pub seq_id: u64,
    #[prost(uint64, required, tag = "2")]
    pub log_id: u64,
    #[prost(int32, required, tag = "3")]
    pub service: i32,
    #[prost(int32, required, tag = "4")]
    pub method: i32,
    #[prost(message, repeated, tag = "5")]
    pub headers: Vec<WsFrameHeader>,
    #[prost(string, optional, tag = "6")]
    pub payload_encoding: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub payload_type: Option<String>,
    #[prost(bytes, optional, tag = "8")]
    pub payload: Option<Vec<u8>>,
    #[prost(string, optional, tag = "9")]
    pub log_id_new: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct WsFrameHeader {
    #[prost(string, required, tag = "1")]
    pub key: String,
    #[prost(string, required, tag = "2")]
    pub value: String,
}

/// Frame method constants.
const WS_METHOD_CONTROL: i32 = 0;
const WS_METHOD_DATA: i32 = 1;

/// Response from POST /callback/ws/endpoint.
#[derive(Debug, Deserialize)]
struct WsEndpointResponse {
    code: i64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    data: Option<WsEndpointData>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct WsEndpointData {
    URL: String,
    #[serde(default)]
    ClientConfig: Option<WsClientConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
struct WsClientConfig {
    #[serde(default = "default_reconnect_count")]
    ReconnectCount: i32,
    #[serde(default = "default_reconnect_interval")]
    ReconnectInterval: u64,
    #[serde(default = "default_reconnect_nonce")]
    ReconnectNonce: u64,
    #[serde(default = "default_ping_interval")]
    PingInterval: u64,
}

fn default_reconnect_count() -> i32 { -1 }
fn default_reconnect_interval() -> u64 { 120 }
fn default_reconnect_nonce() -> u64 { 30 }
fn default_ping_interval() -> u64 { 120 }

impl Default for WsClientConfig {
    fn default() -> Self {
        Self {
            ReconnectCount: -1,
            ReconnectInterval: 120,
            ReconnectNonce: 30,
            PingInterval: 120,
        }
    }
}
```

- [ ] **Step 2: Add imports at top of file**

Add to existing imports:
```rust
use prost::Message as ProstMessage;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMessage;
```

- [ ] **Step 3: Compile check**

Run: `cargo check --manifest-path plugins/feishu-plugin/Cargo.toml 2>&1 | head -20`
Expected: Success (types defined but not yet used — some warnings are OK).

- [ ] **Step 4: Commit**

```bash
git add plugins/feishu-plugin/src/main.rs
git commit -m "feat(feishu): add Protobuf Frame types and WebSocket endpoint types"
```

---

### Task 6: Implement WebSocket helper functions

**Files:**
- Modify: `plugins/feishu-plugin/src/main.rs` (add before the `start_message_listener` method)

- [ ] **Step 1: Add helper function to get frame header value**

```rust
impl WsFrame {
    fn get_header(&self, key: &str) -> Option<&str> {
        self.headers.iter()
            .find(|h| h.key == key)
            .map(|h| h.value.as_str())
    }
}
```

- [ ] **Step 2: Add function to fetch WebSocket endpoint**

Add as a standalone function (not a method on FeishuClient):

```rust
/// Fetch the Feishu WebSocket endpoint URL.
/// NOTE: This endpoint is at the domain root, NOT under /open-apis/.
async fn fetch_ws_endpoint(
    http: &reqwest::Client,
    app_id: &str,
    app_secret: &str,
) -> Result<(String, WsClientConfig), String> {
    let resp = http
        .post("https://open.feishu.cn/callback/ws/endpoint")
        .header("Content-Type", "application/json")
        .header("locale", "zh")
        .json(&serde_json::json!({
            "AppID": app_id,
            "AppSecret": app_secret,
        }))
        .send()
        .await
        .map_err(|e| format!("fetch ws endpoint failed: {e}"))?;

    let data: WsEndpointResponse = resp
        .json()
        .await
        .map_err(|e| format!("parse ws endpoint response: {e}"))?;

    if data.code != 0 {
        return Err(format!("ws endpoint error (code={}): {}", data.code, data.msg));
    }

    let endpoint = data.data.ok_or("missing data in ws endpoint response")?;
    let config = endpoint.ClientConfig.unwrap_or_default();
    Ok((endpoint.URL, config))
}
```

- [ ] **Step 3: Add function to extract `service_id` from URL**

```rust
fn extract_service_id(url: &str) -> i32 {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "service_id")
                .and_then(|(_, v)| v.parse().ok())
        })
        .unwrap_or(0)
}
```

Note: Add `url = "2"` to Cargo.toml dependencies, OR parse manually:

```rust
fn extract_service_id(url: &str) -> i32 {
    // Simple query param extraction without url crate
    if let Some(query) = url.split('?').nth(1) {
        for pair in query.split('&') {
            if let Some(val) = pair.strip_prefix("service_id=") {
                return val.parse().unwrap_or(0);
            }
        }
    }
    0
}
```

Use the manual approach to avoid adding another dependency.

- [ ] **Step 4: Add function to build a ping frame**

```rust
fn build_ping_frame(service_id: i32) -> Vec<u8> {
    let frame = WsFrame {
        seq_id: 0,
        log_id: 0,
        service: service_id,
        method: WS_METHOD_CONTROL,
        headers: vec![WsFrameHeader {
            key: "type".to_string(),
            value: "ping".to_string(),
        }],
        payload_encoding: None,
        payload_type: None,
        payload: None,
        log_id_new: None,
    };
    frame.encode_to_vec()
}
```

- [ ] **Step 5: Add function to build an ACK frame**

```rust
fn build_ack_frame(original: &WsFrame, code: i32) -> Vec<u8> {
    let ack_payload = serde_json::json!({
        "code": code,
        "headers": {},
        "data": ""
    });
    let mut frame = original.clone();
    frame.payload = Some(ack_payload.to_string().into_bytes());
    // Add biz_rt header
    frame.headers.push(WsFrameHeader {
        key: "biz_rt".to_string(),
        value: "0".to_string(),
    });
    frame.encode_to_vec()
}
```

- [ ] **Step 6: Compile check**

Run: `cargo check --manifest-path plugins/feishu-plugin/Cargo.toml 2>&1 | head -20`
Expected: Success (some unused warnings OK).

- [ ] **Step 7: Commit**

```bash
git add plugins/feishu-plugin/src/main.rs
git commit -m "feat(feishu): add WebSocket helper functions (endpoint, ping, ack, service_id)"
```

---

### Task 7: Implement `start_message_listener` with WebSocket long connection

**Files:**
- Modify: `plugins/feishu-plugin/src/main.rs:517-531` (replace stub) and `~687-693` (fix call site)

- [ ] **Step 1: Replace the stub `start_message_listener` method**

Delete the existing method (lines 517-531) and replace it with a standalone async function:

```rust
/// Run the Feishu WebSocket long connection loop.
///
/// This is a standalone function (not a method on FeishuClient) to avoid
/// holding the Mutex lock. Takes owned copies of credentials.
async fn run_ws_message_listener(
    app_id: String,
    app_secret: String,
    http: reqwest::Client,
    tx: mpsc::Sender<JsonRpcNotification>,
) {
    info!("feishu: starting WebSocket long connection listener");

    let mut config = WsClientConfig::default();
    let mut attempt = 0u32;

    loop {
        // 1. Fetch WSS endpoint
        let (ws_url, new_config) = match fetch_ws_endpoint(&http, &app_id, &app_secret).await {
            Ok(result) => {
                attempt = 0;
                result
            }
            Err(e) => {
                // Auth errors (514/403) are fatal
                if e.contains("code=514") || e.contains("code=403") {
                    error!("feishu: WebSocket auth failed (fatal): {e}");
                    let _ = tx.send(JsonRpcNotification::new(
                        "im/on_status",
                        Some(serde_json::json!({
                            "status": "error",
                            "message": format!("WebSocket auth failed: {e}"),
                        })),
                    )).await;
                    return;
                }
                warn!("feishu: failed to get ws endpoint: {e}, will retry");
                tokio::time::sleep(tokio::time::Duration::from_secs(config.ReconnectInterval)).await;
                continue;
            }
        };
        config = new_config;

        let service_id = extract_service_id(&ws_url);
        info!(url = %ws_url, service_id, "feishu: connecting WebSocket");

        // 2. Connect WebSocket
        let ws_stream = match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((stream, _response)) => stream,
            Err(e) => {
                warn!("feishu: WebSocket connect failed: {e}");
                if !reconnect_wait(&config, &mut attempt).await { return; }
                continue;
            }
        };

        info!("feishu: WebSocket connected");
        let _ = tx.send(JsonRpcNotification::new(
            "im/on_status",
            Some(serde_json::json!({"status": "connected"})),
        )).await;

        // 3. Run the connection
        let disconnect_reason = run_ws_session(ws_stream, service_id, &config, &tx).await;
        warn!(reason = %disconnect_reason, "feishu: WebSocket disconnected");

        let _ = tx.send(JsonRpcNotification::new(
            "im/on_status",
            Some(serde_json::json!({
                "status": "disconnected",
                "message": disconnect_reason,
            })),
        )).await;

        // 4. Reconnect with jitter
        if !reconnect_wait(&config, &mut attempt).await { return; }
    }
}

/// Returns `true` if reconnection should continue, `false` if max attempts exceeded.
async fn reconnect_wait(config: &WsClientConfig, attempt: &mut u32) -> bool {
    *attempt += 1;
    if config.ReconnectCount >= 0 && *attempt as i32 > config.ReconnectCount {
        error!("feishu: max reconnect attempts reached, giving up");
        return false;
    }
    let jitter_ms = rand::random::<u64>() % (config.ReconnectNonce * 1000);
    let wait = tokio::time::Duration::from_millis(jitter_ms);
    info!(attempt = *attempt, wait_ms = jitter_ms, "feishu: reconnecting after jitter");
    tokio::time::sleep(wait).await;
    true
}

async fn run_ws_session(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    service_id: i32,
    config: &WsClientConfig,
    tx: &mpsc::Sender<JsonRpcNotification>,
) -> String {
    let (mut write, mut read) = ws_stream.split();

    let ping_interval = tokio::time::Duration::from_secs(config.PingInterval);
    let timeout = tokio::time::Duration::from_secs(300);
    let mut last_recv = tokio::time::Instant::now();
    let mut ping_ticker = tokio::time::interval(ping_interval);
    ping_ticker.tick().await; // skip first immediate tick

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(WsMessage::Binary(data))) => {
                        last_recv = tokio::time::Instant::now();
                        match WsFrame::decode(data.as_ref()) {
                            Ok(frame) => {
                                if frame.method == WS_METHOD_CONTROL {
                                    // Pong — could update config from payload
                                    if frame.get_header("type") == Some("pong") {
                                        debug!("feishu: received pong");
                                    }
                                } else if frame.method == WS_METHOD_DATA {
                                    // Event or card
                                    handle_data_frame(&frame, &mut write, tx).await;
                                }
                            }
                            Err(e) => {
                                warn!("feishu: failed to decode protobuf frame: {e}");
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) => {
                        return "server closed connection".to_string();
                    }
                    Some(Ok(_)) => {
                        // Text or Ping/Pong at WS level — ignore
                        last_recv = tokio::time::Instant::now();
                    }
                    Some(Err(e)) => {
                        return format!("WebSocket error: {e}");
                    }
                    None => {
                        return "WebSocket stream ended".to_string();
                    }
                }
            }
            _ = ping_ticker.tick() => {
                let ping_data = build_ping_frame(service_id);
                if let Err(e) = write.send(WsMessage::Binary(ping_data.into())).await {
                    return format!("failed to send ping: {e}");
                }
                debug!("feishu: sent ping");

                // Check timeout
                if last_recv.elapsed() > timeout {
                    return "heartbeat timeout (300s)".to_string();
                }
            }
        }
    }
}

async fn handle_data_frame(
    frame: &WsFrame,
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        WsMessage,
    >,
    tx: &mpsc::Sender<JsonRpcNotification>,
) {
    let header_type = frame.get_header("type").unwrap_or("");

    // Send ACK immediately
    let ack_data = build_ack_frame(frame, 200);
    if let Err(e) = write.send(WsMessage::Binary(ack_data.into())).await {
        warn!("feishu: failed to send ACK: {e}");
    }

    if header_type != "event" {
        debug!(header_type, "feishu: ignoring non-event data frame");
        return;
    }

    // Parse event payload
    let payload = match &frame.payload {
        Some(p) => p,
        None => {
            warn!("feishu: event frame has no payload");
            return;
        }
    };

    let event: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(v) => v,
        Err(e) => {
            warn!("feishu: failed to parse event JSON: {e}");
            return;
        }
    };

    let event_type = event
        .pointer("/header/event_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if event_type != "im.message.receive_v1" {
        debug!(event_type, "feishu: ignoring non-message event");
        return;
    }

    // Extract message fields
    let chat_id = event.pointer("/event/message/chat_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let message_type = event.pointer("/event/message/message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text");
    let content_str = event.pointer("/event/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sender_id = event.pointer("/event/sender/sender_id/open_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let create_time: u64 = event.pointer("/event/message/create_time")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let message_id = event.pointer("/event/message/message_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Extract text from content JSON (e.g., {"text": "hello"})
    let text = if message_type == "text" {
        serde_json::from_str::<serde_json::Value>(content_str)
            .ok()
            .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| content_str.to_string())
    } else {
        format!("[{message_type}] {content_str}")
    };

    info!(
        chat_id,
        sender_id,
        message_id,
        message_type,
        "feishu: received message"
    );

    // Send im/on_message notification to host
    let _ = tx.send(JsonRpcNotification::new(
        "im/on_message",
        Some(serde_json::json!({
            "text": text,
            "sender": sender_id,
            "channel": chat_id,
            "timestamp": create_time,
        })),
    )).await;
}
```

- [ ] **Step 2: Fix the call site in `handle_initialize`**

Replace lines 687-693:
```rust
// OLD:
// let client_clone = Arc::clone(client);
// let notif_tx = notification_tx.clone();
// tokio::spawn(async move {
//     let feishu = client_clone.lock().await;
//     feishu.start_message_listener(notif_tx).await;
// });

// NEW: Extract credentials and spawn without holding the Mutex
let notif_tx = notification_tx.clone();
{
    let feishu = client.lock().await;
    if !feishu.dry_run {
        let app_id = feishu.app_id.clone();
        let app_secret = feishu.app_secret.clone();
        let http = feishu.http.clone();
        tokio::spawn(async move {
            run_ws_message_listener(app_id, app_secret, http, notif_tx).await;
        });
    } else {
        info!("feishu: dry-run mode, skipping WebSocket listener");
    }
}
```

Note: The `dry_run`, `app_id`, `app_secret`, and `http` fields on `FeishuClient` are currently private. They are accessed within the same file (both `FeishuClient` and `run_ws_message_listener` are in `main.rs`), so this works without visibility changes.

- [ ] **Step 3: Remove the old `start_message_listener` method from `FeishuClient`**

Delete the old method (lines 517-531). It is replaced by the standalone `run_ws_message_listener` function.

- [ ] **Step 4: Compile check**

Run: `cargo check --manifest-path plugins/feishu-plugin/Cargo.toml 2>&1 | head -30`
Expected: Success.

- [ ] **Step 5: Commit**

```bash
git add plugins/feishu-plugin/src/main.rs
git commit -m "feat(feishu): implement WebSocket long connection for receiving messages"
```

---

### Task 8: Build and install plugins, smoke test

**Files:** None (testing only)

- [ ] **Step 1: Build all plugins**

Run: `make plugins 2>&1 | tail -10`
Expected: Both plugins compile successfully.

- [ ] **Step 2: Install plugins**

Run: `make install-plugins 2>&1`
Expected: Plugins copied to `~/.rtb/plugins/`.

- [ ] **Step 3: Build the main project**

Run: `cargo build -p rtb-cli 2>&1 | tail -10`
Expected: Success.

- [ ] **Step 4: Commit any Cargo.lock changes**

```bash
git add plugins/feishu-plugin/Cargo.lock
git commit -m "chore: update feishu-plugin Cargo.lock"
```
