use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct StartTunnelRequest {
    /// Tunnel provider (e.g., "cloudflare").
    #[serde(default)]
    pub provider: Option<String>,
    /// Custom domain.
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Serialize)]
pub struct TunnelStatusResponse {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
struct MessageBody {
    message: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/tunnel/status — get tunnel status.
pub async fn tunnel_status(State(state): State<AppState>) -> impl IntoResponse {
    // Read the current tunnel URL from shared state
    let current_url = state.tunnel_url.read().await.clone();

    match &state.plugin_manager {
        Some(pm) => {
            // Check if any tunnel plugin is running
            let plugins = pm.list_plugins().await;
            let tunnel_plugin = plugins.iter().find(|(_id, _name, _state)| {
                // Look for plugins with "tunnel" in the name/id
                _id.contains("tunnel") || _name.to_lowercase().contains("tunnel")
            });

            match tunnel_plugin {
                Some((id, name, plugin_state)) => {
                    let is_active = plugin_state.to_string() == "ready";
                    Json(TunnelStatusResponse {
                        active: is_active && current_url.is_some(),
                        provider: Some(name.clone()),
                        url: current_url,
                        message: format!("Tunnel plugin '{}' is {}", id, plugin_state),
                    })
                    .into_response()
                }
                None => Json(TunnelStatusResponse {
                    active: false,
                    provider: None,
                    url: None,
                    message: "No tunnel plugin installed".to_string(),
                })
                .into_response(),
            }
        }
        None => Json(TunnelStatusResponse {
            active: false,
            provider: None,
            url: None,
            message: "Plugin manager not available".to_string(),
        })
        .into_response(),
    }
}

/// POST /api/v1/tunnel/start — start a tunnel.
pub async fn start_tunnel(
    State(state): State<AppState>,
    Json(body): Json<StartTunnelRequest>,
) -> impl IntoResponse {
    match &state.plugin_manager {
        Some(pm) => {
            // Find a tunnel plugin
            let plugins = pm.list_plugins().await;
            let tunnel_plugin = plugins.iter().find(|(id, _name, _state)| {
                if let Some(ref provider) = body.provider {
                    id.contains(provider)
                } else {
                    id.contains("tunnel")
                }
            });

            match tunnel_plugin {
                Some((id, _name, _state)) => {
                    let params = serde_json::json!({
                        "domain": body.domain,
                    });
                    match pm.call_plugin(id, "tunnel/start", Some(params)).await {
                        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
                        Err(e) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorBody {
                                error: e.to_string(),
                            }),
                        )
                            .into_response(),
                    }
                }
                None => (
                    StatusCode::NOT_FOUND,
                    Json(ErrorBody {
                        error: "No tunnel plugin found. Install a tunnel plugin first.".to_string(),
                    }),
                )
                    .into_response(),
            }
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody {
                error: "Plugin manager not available".to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /api/v1/tunnel/stop — stop the tunnel.
pub async fn stop_tunnel(State(state): State<AppState>) -> impl IntoResponse {
    match &state.plugin_manager {
        Some(pm) => {
            let plugins = pm.list_plugins().await;
            let tunnel_plugin = plugins
                .iter()
                .find(|(id, _name, _state)| id.contains("tunnel"));

            match tunnel_plugin {
                Some((id, _name, _state)) => match pm.call_plugin(id, "tunnel/stop", None).await {
                    Ok(_) => (
                        StatusCode::OK,
                        Json(MessageBody {
                            message: "Tunnel stopped".to_string(),
                        }),
                    )
                        .into_response(),
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorBody {
                            error: e.to_string(),
                        }),
                    )
                        .into_response(),
                },
                None => (
                    StatusCode::NOT_FOUND,
                    Json(ErrorBody {
                        error: "No tunnel plugin found".to_string(),
                    }),
                )
                    .into_response(),
            }
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody {
                error: "Plugin manager not available".to_string(),
            }),
        )
            .into_response(),
    }
}
