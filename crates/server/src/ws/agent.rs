//! WebSocket handler for agent (ACP) sessions.
//!
//! Bridges the browser client to an ACP agent subprocess via the AgentManager.
//! Client sends JSON commands (message); server forwards
//! DataEvents (AgentText, AgentToolUse, etc.) as JSON to the client.

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
use rtb_core::agent::manager::agent_event_to_data_event;
use rtb_core::events::DataEvent;

/// Query parameters for the agent WebSocket endpoint.
#[derive(Debug, Deserialize)]
pub struct AgentParams {
    pub session: String,
    pub token: String,
    /// If provided, the client wants events starting after this sequence number.
    pub last_seq: Option<u64>,
}

/// JSON commands sent from the client to the server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AgentClientCommand {
    /// Send a user message to the agent.
    #[serde(rename = "message")]
    Message { text: String },
    /// Cancel the current agent turn.
    #[serde(rename = "cancel")]
    Cancel,
}

/// Agent WebSocket upgrade handler.
///
/// Validates the token and session, then upgrades to a WebSocket that bridges
/// the browser client to the ACP agent subprocess.
pub async fn ws_agent(
    ws: WebSocketUpgrade,
    Query(params): Query<AgentParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // 1. Validate token
    let expected = state.token.read().await.clone();
    if params.token != expected {
        return (axum::http::StatusCode::UNAUTHORIZED, "Invalid token").into_response();
    }

    // 2. Lookup session in AgentManager
    let session_id = params.session.clone();
    if !state.core.agent_manager.has_agent(&session_id) {
        return (
            axum::http::StatusCode::NOT_FOUND,
            "Agent session not found",
        )
            .into_response();
    }

    let last_seq = params.last_seq;

    // 3. Upgrade to WebSocket
    ws.on_upgrade(move |socket| handle_agent(socket, state, session_id, last_seq))
}

/// Main agent WebSocket loop after upgrade.
async fn handle_agent(
    socket: WebSocket,
    state: AppState,
    session_id: String,
    _last_seq: Option<u64>,
) {
    info!(session_id = %session_id, "agent WebSocket connected");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Subscribe to session data channel via EventBus
    let mut data_rx = state.core.event_bus.create_data_subscriber(&session_id);

    // Send initial status message to the client
    let status_msg = serde_json::json!({
        "type": "status",
        "status": "connected",
        "session_id": session_id,
    });
    if ws_tx
        .send(Message::Text(status_msg.to_string().into()))
        .await
        .is_err()
    {
        warn!(session_id = %session_id, "failed to send initial status, closing");
        return;
    }

    // Replay event history for reconnecting clients
    let history = state.core.agent_manager.get_event_history(&session_id);
    if !history.is_empty() {
        debug!(session_id = %session_id, count = history.len(), "replaying event history");
        for (i, event) in history.iter().enumerate() {
            let seq = (i + 1) as u64;
            let data_event = agent_event_to_data_event(seq, event);
            let msg = data_event_to_json(&data_event);
            if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                warn!(session_id = %session_id, "failed to send replay event, closing");
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
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<AgentClientCommand>(&text) {
                            Ok(AgentClientCommand::Message { text: user_text }) => {
                                debug!(session_id = %session_id, "received user message");
                                match state.core.agent_manager.send_message(&session_id, user_text).await {
                                    Ok(()) => {}
                                    Err(e) => {
                                        error!(session_id = %session_id, error = %e, "failed to send message to agent");
                                        let err_msg = serde_json::json!({
                                            "type": "error",
                                            "seq": 0,
                                            "message": e.to_string(),
                                            "severity": "transient",
                                            "guidance": "Try sending the message again.",
                                        });
                                        let _ = ws_tx.send(Message::Text(err_msg.to_string().into())).await;
                                    }
                                }
                            }
                            Ok(AgentClientCommand::Cancel) => {
                                debug!(session_id = %session_id, "received cancel request");
                                // Cancel is best-effort; agent may already have finished
                                warn!(session_id = %session_id, "cancel not yet implemented in agent manager");
                            }
                            Err(e) => {
                                warn!(session_id = %session_id, error = %e, text = %text, "unknown agent client command");
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
                        debug!(session_id = %session_id, "agent WebSocket closed by client");
                        break;
                    }
                    Some(Ok(Message::Binary(_))) => {
                        // Binary frames not expected for agent sessions
                        warn!(session_id = %session_id, "unexpected binary frame on agent WebSocket");
                    }
                    Some(Err(e)) => {
                        warn!(session_id = %session_id, error = %e, "agent WebSocket error");
                        break;
                    }
                }
            }

            // Outgoing data event from the agent
            event = data_rx.recv() => {
                match event {
                    Some(DataEvent::AgentText { seq, content, streaming }) => {
                        let msg = serde_json::json!({
                            "type": "text",
                            "seq": seq,
                            "content": content,
                            "streaming": streaming,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send text, closing");
                            break;
                        }
                    }
                    Some(DataEvent::AgentThinking { seq, content }) => {
                        let msg = serde_json::json!({
                            "type": "thinking",
                            "seq": seq,
                            "content": content,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send thinking, closing");
                            break;
                        }
                    }
                    Some(DataEvent::AgentToolUse { seq, id, name, input }) => {
                        let msg = serde_json::json!({
                            "type": "tool_use",
                            "seq": seq,
                            "id": id,
                            "name": name,
                            "input": input,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send tool_use, closing");
                            break;
                        }
                    }
                    Some(DataEvent::AgentToolResult { seq, id, output, is_error }) => {
                        let msg = serde_json::json!({
                            "type": "tool_result",
                            "seq": seq,
                            "id": id,
                            "output": output,
                            "is_error": is_error,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send tool_result, closing");
                            break;
                        }
                    }
                    Some(DataEvent::AgentProgress { seq, message }) => {
                        let msg = serde_json::json!({
                            "type": "progress",
                            "seq": seq,
                            "message": message,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send progress, closing");
                            break;
                        }
                    }
                    Some(DataEvent::AgentTurnComplete { seq, cost_usd }) => {
                        let msg = serde_json::json!({
                            "type": "turn_complete",
                            "seq": seq,
                            "cost_usd": cost_usd,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send turn_complete, closing");
                            break;
                        }
                    }
                    Some(DataEvent::AgentError { seq, message, severity, guidance }) => {
                        let severity_str = match severity {
                            rtb_core::events::ErrorClass::Transient => "transient",
                            rtb_core::events::ErrorClass::Permanent => "permanent",
                        };
                        let msg = serde_json::json!({
                            "type": "error",
                            "seq": seq,
                            "message": message,
                            "severity": severity_str,
                            "guidance": guidance,
                        });
                        if ws_tx.send(Message::Text(msg.to_string().into())).await.is_err() {
                            debug!(session_id = %session_id, "failed to send error, closing");
                            break;
                        }
                    }
                    Some(_) => {
                        // Ignore non-agent data events (PTY output, etc.)
                    }
                    None => {
                        // Data channel closed (session removed or agent exited)
                        debug!(session_id = %session_id, "agent data channel closed");
                        let exit_msg = serde_json::json!({
                            "type": "status",
                            "status": "disconnected",
                            "session_id": session_id,
                        });
                        let _ = ws_tx.send(Message::Text(exit_msg.to_string().into())).await;
                        let _ = ws_tx.send(Message::Close(Some(CloseFrame {
                            code: 1000,
                            reason: "Agent session ended".into(),
                        }))).await;
                        break;
                    }
                }
            }

            // Heartbeat ping
            _ = ping_interval.tick() => {
                if ws_tx.send(Message::Ping(vec![].into())).await.is_err() {
                    debug!(session_id = %session_id, "ping failed, closing agent WebSocket");
                    break;
                }
            }
        }
    }

    info!(session_id = %session_id, "agent WebSocket disconnected");
}

/// Convert a DataEvent to a JSON value for sending over WebSocket.
/// Used both for live events and history replay.
fn data_event_to_json(event: &DataEvent) -> serde_json::Value {
    match event {
        DataEvent::AgentText { seq, content, streaming } => serde_json::json!({
            "type": "text",
            "seq": seq,
            "content": content,
            "streaming": streaming,
        }),
        DataEvent::AgentThinking { seq, content } => serde_json::json!({
            "type": "thinking",
            "seq": seq,
            "content": content,
        }),
        DataEvent::AgentToolUse { seq, id, name, input } => serde_json::json!({
            "type": "tool_use",
            "seq": seq,
            "id": id,
            "name": name,
            "input": input,
        }),
        DataEvent::AgentToolResult { seq, id, output, is_error } => serde_json::json!({
            "type": "tool_result",
            "seq": seq,
            "id": id,
            "output": output,
            "is_error": is_error,
        }),
        DataEvent::AgentProgress { seq, message } => serde_json::json!({
            "type": "progress",
            "seq": seq,
            "message": message,
        }),
        DataEvent::AgentTurnComplete { seq, cost_usd } => serde_json::json!({
            "type": "turn_complete",
            "seq": seq,
            "cost_usd": cost_usd,
        }),
        DataEvent::AgentError { seq, message, severity, guidance } => {
            let severity_str = match severity {
                rtb_core::events::ErrorClass::Transient => "transient",
                rtb_core::events::ErrorClass::Permanent => "permanent",
            };
            serde_json::json!({
                "type": "error",
                "seq": seq,
                "message": message,
                "severity": severity_str,
                "guidance": guidance,
            })
        }
        _ => serde_json::json!({ "type": "unknown" }),
    }
}
