# Tunnel Port Passthrough & Feishu WebSocket Long Connection

**Date:** 2026-03-26
**Status:** Approved

## Problem

Two issues in the plugin subsystem:

1. **Cloudflare Tunnel** hardcodes `local_port: 3000` in `PluginManager` instead of using the actual server port from config. Additionally, the Tauri embedded daemon does not subscribe to `TunnelReady`/`TunnelDown` events, so `AppState.tunnel_url` is never updated.

2. **Feishu Plugin** has a stub `start_message_listener()` that sleeps forever. It can only send messages (via REST API) but cannot receive them. The user wants WebSocket long connection mode (client-initiated, no public domain required).

## Design

### Part 1: Cloudflare Tunnel Port Passthrough

**Goal:** Pass the actual `config.server.port` to the tunnel plugin instead of hardcoding 3000; fix Tauri tunnel URL tracking.

#### Changes

1. **`PluginManager` struct** (`crates/plugin-host/src/manager.rs`):
   - Add `server_port: u16` field
   - Update `new()` signature: `new(plugins_dir, event_bus, timeout_secs, server_port)`
   - Replace hardcoded `3000` in `tunnel/initialize` and `tunnel/start` params with `self.server_port`

2. **CLI daemon** (`crates/cli/src/commands/start.rs`):
   - Pass `config.server.port` to `PluginManager::new()`

3. **Tauri daemon** (`crates/tauri-app/src/commands.rs`):
   - Pass `config.server.port` to `PluginManager::new()`
   - Add `TunnelReady`/`TunnelDown` event subscriber (identical to CLI's `start.rs:262-289`) that updates `AppState.tunnel_url`

### Part 2: Feishu WebSocket Long Connection

**Goal:** Implement Feishu's long connection protocol so the plugin can receive messages in real-time without a public domain.

#### Protocol Summary

Feishu's long connection is a two-step process:

1. **REST call** to `POST https://open.feishu.cn/callback/ws/endpoint` with `AppID`/`AppSecret` to get a WSS URL with embedded auth token
2. **WebSocket connection** using that URL; messages arrive as Protobuf-encoded binary frames

Frame types:
- `method=0` (Control): ping/pong heartbeat
- `method=1` (Data): events (im.message.receive_v1, etc.)

Key constraints:
- ACK must be sent within 3 seconds of receiving a data frame
- Ping interval: 120s (server-configurable via pong response)
- Connection timeout: 300s with no frames received
- Reconnect with random jitter + configurable interval
- Max 50 concurrent connections per app

#### New Dependencies (`plugins/feishu-plugin/Cargo.toml`)

- `prost` — Protobuf encoding/decoding
- `tokio-tungstenite` + `tungstenite` — WebSocket client
- `futures-util` — stream utilities for WebSocket read/write split

#### Protobuf Structures

Defined as `prost` derive structs (no .proto file or build.rs needed):

```rust
#[derive(Clone, PartialEq, prost::Message)]
pub struct Frame {
    #[prost(uint64, required, tag = "1")]
    pub seq_id: u64,
    #[prost(uint64, required, tag = "2")]
    pub log_id: u64,
    #[prost(int32, required, tag = "3")]
    pub service: i32,
    #[prost(int32, required, tag = "4")]
    pub method: i32,
    #[prost(message, repeated, tag = "5")]
    pub headers: Vec<FrameHeader>,
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
pub struct FrameHeader {
    #[prost(string, required, tag = "1")]
    pub key: String,
    #[prost(string, required, tag = "2")]
    pub value: String,
}
```

#### Implementation in `start_message_listener()`

Replace the stub with a connection loop:

```
loop {
    1. POST /callback/ws/endpoint → get WSS URL + ClientConfig
    2. Extract service_id from URL query params
    3. Connect WebSocket to WSS URL
    4. Check handshake response headers for errors
    5. Split into read/write halves
    6. Spawn concurrent tasks:
       a. Ping loop: send ping frame every PingInterval seconds
       b. Receive loop:
          - Decode Protobuf Frame from binary message
          - If control frame (method=0): handle pong (update ClientConfig)
          - If data frame (method=1):
            - Parse event payload (JSON)
            - If im.message.receive_v1: extract chat_id, text, sender
            - Send ACK frame back within 3s
            - Send im/on_message notification to host via tx channel
       c. Timeout monitor: break if 300s with no frames
    7. On disconnect: random jitter wait → retry from step 1
       - On auth errors (514/403): stop retrying
```

#### Notification Format

The `im/on_message` notification sent to the host:

```json
{
    "method": "im/on_message",
    "params": {
        "channel": "<chat_id>",
        "text": "<extracted text content>",
        "sender": "<open_id>",
        "message_id": "<message_id>",
        "message_type": "<text|image|post|...>"
    }
}
```

This is already handled by the existing `ImBridge` command parser — no changes needed in plugin-host.

#### What Is NOT Implemented

- **Fragment reassembly**: RTB messages are small, won't hit Feishu's fragmentation threshold
- **Card callbacks**: Not needed for IM use case
- **Encryption/signature verification**: Long connection mode delivers plaintext events per Feishu docs
- **Changes to IM Bridge or plugin-host interfaces**: Existing `im/on_message` handling is sufficient

## Files Changed

| File | Change |
|------|--------|
| `crates/plugin-host/src/manager.rs` | Add `server_port` field, use in tunnel params |
| `crates/cli/src/commands/start.rs` | Pass `config.server.port` to PluginManager |
| `crates/tauri-app/src/commands.rs` | Pass port + add tunnel URL event subscriber |
| `plugins/feishu-plugin/Cargo.toml` | Add prost, tokio-tungstenite, futures-util |
| `plugins/feishu-plugin/src/main.rs` | Add proto structs, rewrite start_message_listener |
