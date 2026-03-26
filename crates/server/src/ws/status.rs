use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

use crate::state::AppState;
use rtb_core::events::ControlEvent;

/// Query parameters for the status WebSocket endpoint.
#[derive(Debug, Deserialize)]
pub struct StatusParams {
    pub token: String,
}

/// Status WebSocket upgrade handler.
///
/// Pushes control events (session lifecycle, agent status, tunnel, plugin) to
/// clients for real-time UI updates.
pub async fn ws_status(
    ws: WebSocketUpgrade,
    Query(params): Query<StatusParams>,
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

    // 3. Upgrade to WebSocket
    ws.on_upgrade(move |socket| handle_status(socket, state))
}

/// Main status WebSocket loop after upgrade.
async fn handle_status(socket: WebSocket, state: AppState) {
    info!("status WebSocket connected");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // 2. Subscribe to EventBus control channel
    let mut control_rx = state.core.event_bus.subscribe_control();

    // Heartbeat: ping every 30 seconds
    let mut ping_interval = interval(Duration::from_secs(30));
    // Skip the first immediate tick
    ping_interval.tick().await;

    loop {
        tokio::select! {
            // Incoming messages from client (we only expect pong and close)
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_tx.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Pong received, connection is alive
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        debug!("status WebSocket closed by client");
                        break;
                    }
                    Some(Ok(Message::Text(text))) => {
                        // Handle application-level ping from frontend
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("ping") {
                                let pong = serde_json::json!({ "type": "pong" });
                                let _ = ws_tx.send(Message::Text(pong.to_string().into())).await;
                            }
                        }
                    }
                    Some(Ok(_)) => {
                        // Ignore other binary messages from client on status channel
                    }
                    Some(Err(e)) => {
                        warn!(error = %e, "status WebSocket error");
                        break;
                    }
                }
            }

            // Outgoing control event
            event = control_rx.recv() => {
                match event {
                    Ok(event) => {
                        if let Some(json) = control_event_to_json(&event) {
                            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                                debug!("failed to send status event, closing");
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "status subscriber lagged, some events were missed");
                        // Continue receiving — we just lost some events
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("control channel closed, closing status WebSocket");
                        break;
                    }
                }
            }

            // Heartbeat ping
            _ = ping_interval.tick() => {
                if ws_tx.send(Message::Ping(vec![].into())).await.is_err() {
                    debug!("status ping failed, closing WebSocket");
                    break;
                }
            }
        }
    }

    info!("status WebSocket disconnected");
}

/// Convert a ControlEvent into a JSON string for the WebSocket client.
/// Returns None for events that should not be forwarded.
fn control_event_to_json(event: &ControlEvent) -> Option<String> {
    let json = match event {
        ControlEvent::SessionCreated {
            session_id,
            session_type,
        } => {
            let st = match session_type {
                rtb_core::events::SessionType::Terminal => "terminal",
                rtb_core::events::SessionType::Agent => "agent",
            };
            serde_json::json!({
                "type": "session_created",
                "session_id": session_id,
                "session_type": st,
            })
        }
        ControlEvent::SessionDeleted { session_id } => {
            serde_json::json!({
                "type": "session_deleted",
                "session_id": session_id,
            })
        }
        ControlEvent::SessionSwitched { session_id } => {
            serde_json::json!({
                "type": "session_switched",
                "session_id": session_id,
            })
        }
        ControlEvent::AgentStatusChanged {
            session_id,
            status,
        } => {
            let status_str = match status {
                rtb_core::events::AgentStatus::Initializing => "initializing",
                rtb_core::events::AgentStatus::Ready => "ready",
                rtb_core::events::AgentStatus::Working => "working",
                rtb_core::events::AgentStatus::WaitingApproval => "waiting_approval",
                rtb_core::events::AgentStatus::Idle => "idle",
                rtb_core::events::AgentStatus::Crashed { .. } => "crashed",
            };
            serde_json::json!({
                "type": "agent_status",
                "session_id": session_id,
                "status": status_str,
            })
        }
        ControlEvent::AgentError {
            session_id,
            error,
            class,
        } => {
            let class_str = match class {
                rtb_core::events::ErrorClass::Transient => "transient",
                rtb_core::events::ErrorClass::Permanent => "permanent",
            };
            serde_json::json!({
                "type": "agent_error",
                "session_id": session_id,
                "error": error,
                "class": class_str,
            })
        }
        ControlEvent::TunnelReady { url } => {
            serde_json::json!({
                "type": "tunnel_ready",
                "url": url,
            })
        }
        ControlEvent::TunnelDown { reason } => {
            serde_json::json!({
                "type": "tunnel_down",
                "reason": reason,
            })
        }
        ControlEvent::PluginLoaded { plugin_id, name } => {
            serde_json::json!({
                "type": "plugin_loaded",
                "plugin_id": plugin_id,
                "name": name,
            })
        }
        ControlEvent::PluginError { plugin_id, error } => {
            serde_json::json!({
                "type": "plugin_error",
                "plugin_id": plugin_id,
                "error": error,
            })
        }
        ControlEvent::NotificationTriggered {
            session_id,
            trigger_type,
            summary,
            urgent,
        } => {
            serde_json::json!({
                "type": "notification",
                "session_id": session_id,
                "trigger_type": trigger_type,
                "summary": summary,
                "urgent": urgent,
            })
        }
    };

    Some(json.to_string())
}
