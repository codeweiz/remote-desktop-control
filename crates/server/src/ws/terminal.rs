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
use rtb_core::events::DataEvent;

/// Query parameters for the terminal WebSocket endpoint.
#[derive(Debug, Deserialize)]
pub struct TerminalParams {
    pub session: String,
    pub token: String,
    /// If provided, replay missed events from the ring buffer starting after this sequence.
    pub last_seq: Option<u64>,
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

    let last_seq = params.last_seq;

    // 5. Upgrade to WebSocket
    ws.on_upgrade(move |socket| handle_terminal(socket, state, session_id, last_seq))
}

/// Main terminal WebSocket loop after upgrade.
async fn handle_terminal(
    socket: WebSocket,
    state: AppState,
    session_id: String,
    last_seq: Option<u64>,
) {
    info!(session_id = %session_id, last_seq = ?last_seq, "terminal WebSocket connected");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // 3. Subscribe to session data channel via EventBus
    let mut data_rx = state.core.event_bus.create_data_subscriber(&session_id);

    // 4. If last_seq provided, replay missed events from ring buffer.
    //    If the requested seq is older than the ring buffer's oldest entry,
    //    send a replay_gap indicator first so the client knows about the gap.
    if let Some(seq) = last_seq {
        if let Some(session) = state.core.pty_manager.get_session(&session_id) {
            let ring = session.buffer();

            // Detect gap: if the client's last_seq is before the ring buffer's
            // oldest entry, there are events we can no longer replay.
            if let Some(first_available) = ring.first_seq() {
                if seq < first_available.saturating_sub(1) {
                    let gap_msg = serde_json::json!({
                        "type": "replay_gap",
                        "missed_from": seq,
                        "available_from": first_available,
                    });
                    if ws_tx
                        .send(Message::Text(gap_msg.to_string().into()))
                        .await
                        .is_err()
                    {
                        warn!(session_id = %session_id, "failed to send replay_gap, closing");
                        return;
                    }
                    debug!(
                        session_id = %session_id,
                        missed_from = seq,
                        available_from = first_available,
                        "sent replay_gap indicator"
                    );
                }
            }

            let missed = ring.get_since(seq);
            let mut replayed_last_seq = seq;

            for (event_seq, data) in missed {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                let msg = serde_json::json!({
                    "type": "output",
                    "seq": event_seq,
                    "data": b64,
                });
                if ws_tx
                    .send(Message::Text(msg.to_string().into()))
                    .await
                    .is_err()
                {
                    warn!(session_id = %session_id, "failed to send replay data, closing");
                    return;
                }
                replayed_last_seq = event_seq;
            }

            // Send replay_done marker
            let done_msg = serde_json::json!({
                "type": "replay_done",
                "last_seq": replayed_last_seq,
            });
            if ws_tx
                .send(Message::Text(done_msg.to_string().into()))
                .await
                .is_err()
            {
                return;
            }
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

            // Outgoing data event from the PTY
            event = data_rx.recv() => {
                match event {
                    Some(DataEvent::PtyOutput { seq, data }) => {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                        let msg = serde_json::json!({
                            "type": "output",
                            "seq": seq,
                            "data": b64,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send PTY output, closing");
                            break;
                        }
                    }
                    Some(DataEvent::PtyExited { exit_code }) => {
                        let msg = serde_json::json!({
                            "type": "exit",
                            "code": exit_code,
                        });
                        let _ = ws_tx.send(Message::Text(msg.to_string().into())).await;
                        info!(session_id = %session_id, exit_code, "PTY exited, closing WebSocket");
                        break;
                    }
                    Some(_) => {
                        // Ignore other data events (agent messages, etc.)
                    }
                    None => {
                        // Data channel closed (session removed)
                        debug!(session_id = %session_id, "data channel closed, closing WebSocket");
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
