use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    pub status: String,
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

/// GET /api/v1/plugins — list all plugins.
pub async fn list_plugins(State(state): State<AppState>) -> impl IntoResponse {
    match &state.plugin_manager {
        Some(pm) => {
            let plugins = pm.list_plugins().await;
            let infos: Vec<PluginInfo> = plugins
                .into_iter()
                .map(|(id, name, plugin_state)| PluginInfo {
                    id,
                    name,
                    plugin_type: String::new(), // PluginState doesn't carry type info
                    status: plugin_state.to_string(),
                })
                .collect();
            Json(infos).into_response()
        }
        None => {
            // No plugin manager — return empty list
            Json(Vec::<PluginInfo>::new()).into_response()
        }
    }
}

/// POST /api/v1/plugins/{name}/enable — enable (start) a plugin.
pub async fn enable_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match &state.plugin_manager {
        Some(_pm) => {
            // The plugin manager discover + start_plugin is manifest-based.
            // For now, return a stub indicating the plugin would be enabled.
            (
                StatusCode::OK,
                Json(MessageBody {
                    message: format!("Plugin '{}' enable requested (restart daemon to apply)", name),
                }),
            )
                .into_response()
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

/// POST /api/v1/plugins/{name}/disable — disable (stop) a plugin.
pub async fn disable_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match &state.plugin_manager {
        Some(pm) => match pm.stop_plugin(&name).await {
            Ok(()) => (
                StatusCode::OK,
                Json(MessageBody {
                    message: format!("Plugin '{}' disabled", name),
                }),
            )
                .into_response(),
            Err(e) => {
                let status = if e.to_string().contains("not found") {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
                (
                    status,
                    Json(ErrorBody {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody {
                error: "Plugin manager not available".to_string(),
            }),
        )
            .into_response(),
    }
}
