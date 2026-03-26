use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::state::AppState;

/// Query parameters for the terminal WebSocket endpoint.
#[derive(Debug, Deserialize)]
pub struct TerminalParams {
    pub session: String,
    pub token: String,
}

/// JSON command from the client (text frames).
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientCommand {
    #[serde(rename = "resize")]
    Resize { cols: u16, rows: u16 },
}

/// Terminal WebSocket upgrade handler.
///
/// Validates the token and session, then upgrades to a WebSocket that bridges
/// xterm.js in the browser to the PTY process.
pub async fn ws_terminal(
    ws: WebSocketUpgrade,
    Query(params): Query<TerminalParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // 1. Validate token
    let expected = state.token.read().await.clone();
    if params.token != expected {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            "Invalid token",
        )
            .into_response();
    }

    // 2. Lookup session in PtyManager
    let session_id = params.session.clone();
    let session = state.core.pty_manager.get_session(&session_id);
    if session.is_none() {
        return (
            axum::http::StatusCode::NOT_FOUND,
            "Session not found",
        )
            .into_response();
    }

    // 3. Upgrade to WebSocket
    ws.on_upgrade(move |socket| handle_terminal(socket, state, session_id))
}

/// Main terminal WebSocket loop after upgrade.
///
/// Subscribes to the PTY session's live broadcast channel and forwards
/// output to the WebSocket. Client input (binary frames) is written to
/// the PTY's stdin. JSON text frames are parsed as commands (e.g. resize).
///
/// NOTE: This implementation will be fully rewritten in Task 5 to use the
/// new binary WebSocket protocol. For now, it uses base64 encoding to keep
/// the existing frontend working.
async fn handle_terminal(
    socket: WebSocket,
    state: AppState,
    session_id: String,
) {
    info!(session_id = %session_id, "terminal WebSocket connected");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Subscribe to live output from the PTY session's broadcast channel
    let session = match state.core.pty_manager.get_session(&session_id) {
        Some(s) => s,
        None => {
            warn!(session_id = %session_id, "session disappeared before WebSocket setup");
            return;
        }
    };
    let mut live_rx = session.subscribe();

    // Heartbeat: ping every 30 seconds
    let mut ping_interval = interval(Duration::from_secs(30));
    // Skip the first immediate tick
    ping_interval.tick().await;

    loop {
        tokio::select! {
            // Incoming message from the client
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        // Raw PTY input
                        if let Err(e) = state.core.pty_manager.write_input(&session_id, &data) {
                            error!(session_id = %session_id, error = %e, "failed to write to PTY");
                            let _ = ws_tx.send(Message::Close(Some(CloseFrame {
                                code: 1011,
                                reason: "Server error: failed to write to PTY".into(),
                            }))).await;
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        // JSON command (e.g. resize)
                        match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(ClientCommand::Resize { cols, rows }) => {
                                debug!(session_id = %session_id, cols, rows, "resize request");
                                if let Err(e) = state.core.pty_manager.resize(&session_id, cols, rows) {
                                    warn!(session_id = %session_id, error = %e, "failed to resize PTY");
                                }
                            }
                            Err(e) => {
                                warn!(session_id = %session_id, error = %e, text = %text, "unknown client command");
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_tx.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Pong received, connection is alive
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        debug!(session_id = %session_id, "terminal WebSocket closed by client");
                        break;
                    }
                    Some(Err(e)) => {
                        warn!(session_id = %session_id, error = %e, "terminal WebSocket error");
                        break;
                    }
                }
            }

            // Live output from the PTY broadcast channel
            result = live_rx.recv() => {
                match result {
                    Ok(data) => {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                        let msg = serde_json::json!({
                            "type": "output",
                            "data": b64,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send PTY output, closing");
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(session_id = %session_id, skipped = n, "WebSocket consumer lagged, some output lost");
                        // Continue — we'll catch up with the next message
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // PTY session ended — the broadcast sender was dropped
                        debug!(session_id = %session_id, "PTY broadcast channel closed, closing WebSocket");
                        let msg = serde_json::json!({
                            "type": "exit",
                            "code": 0,
                        });
                        let _ = ws_tx.send(Message::Text(msg.to_string().into())).await;
                        break;
                    }
                }
            }

            // Heartbeat ping
            _ = ping_interval.tick() => {
                if ws_tx.send(Message::Ping(vec![].into())).await.is_err() {
                    debug!(session_id = %session_id, "ping failed, closing WebSocket");
                    break;
                }
            }
        }
    }

    info!(session_id = %session_id, "terminal WebSocket disconnected");
}
