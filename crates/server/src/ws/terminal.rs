use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
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
    #[serde(rename = "keepalive")]
    Keepalive {
        #[allow(dead_code)]
        client_time: Option<i64>,
    },
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
/// - Sends initial screen content via `capture-pane` on connect (anti-reconnect-storm).
/// - Forwards PTY output to client as Binary frames.
/// - Client input (Binary frames) is written to the PTY stdin.
/// - JSON text frames are parsed as commands (resize, keepalive).
/// - Monitors session status for exit notification.
/// - Closes on lag so the client can reconnect cleanly.
async fn handle_terminal(
    socket: WebSocket,
    state: AppState,
    session_id: String,
) {
    info!(session_id = %session_id, "terminal WebSocket connected");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Lookup session and subscribe to channels
    let session = match state.core.pty_manager.get_session(&session_id) {
        Some(s) => s,
        None => {
            warn!(session_id = %session_id, "session disappeared before WebSocket setup");
            return;
        }
    };
    let mut live_rx = session.subscribe();
    let mut status_rx = session.subscribe_status();

    // Capture-pane on connect: send current screen content so a reconnecting
    // client gets a full screen immediately without waiting for new output.
    if let Ok(initial) = rtb_core::pty::tmux::capture_pane(&session_id) {
        if !initial.is_empty() {
            let _ = ws_tx.send(Message::Binary(initial.into())).await;
        }
    }

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
                        // JSON command (resize, keepalive, etc.)
                        match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(ClientCommand::Resize { cols, rows }) => {
                                debug!(session_id = %session_id, cols, rows, "resize request");
                                if let Err(e) = state.core.pty_manager.resize(&session_id, cols, rows) {
                                    warn!(session_id = %session_id, error = %e, "failed to resize PTY");
                                }
                            }
                            Ok(ClientCommand::Keepalive { .. }) => {
                                let ack = serde_json::json!({
                                    "type": "keepalive_ack",
                                    "server_time": chrono::Utc::now().timestamp_millis(),
                                });
                                let _ = ws_tx.send(Message::Text(ack.to_string().into())).await;
                            }
                            Err(e) => {
                                // Unknown text command — log and ignore.
                                // PTY input comes via Binary frames only.
                                warn!(session_id = %session_id, error = %e, text = %text, "unknown client command, ignoring");
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

            // Live output from the PTY broadcast channel — send as Binary frame
            result = live_rx.recv() => {
                match result {
                    Ok(data) => {
                        match tokio::time::timeout(
                            Duration::from_millis(100),
                            ws_tx.send(Message::Binary(data.into()))
                        ).await {
                            Ok(Ok(())) => {}
                            _ => {
                                warn!(session_id = %session_id, "send timeout or error, closing for reconnect");
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(session_id = %session_id, skipped = n, "client lagging, closing for reconnect");
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // PTY session ended — the broadcast sender was dropped
                        debug!(session_id = %session_id, "PTY broadcast channel closed, closing WebSocket");
                        let code = match &*status_rx.borrow() {
                            rtb_core::pty::session::PtyStatus::Exited(c) => *c,
                            _ => 0,
                        };
                        let msg = serde_json::json!({"type": "exit", "code": code});
                        let _ = ws_tx.send(Message::Text(msg.to_string().into())).await;
                        break;
                    }
                }
            }

            // Session status watch — notify client on exit
            _ = status_rx.changed() => {
                let status = status_rx.borrow().clone();
                if let rtb_core::pty::session::PtyStatus::Exited(code) = status {
                    let msg = serde_json::json!({"type": "exit", "code": code});
                    let _ = ws_tx.send(Message::Text(msg.to_string().into())).await;
                    break;
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
