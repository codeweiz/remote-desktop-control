use std::path::PathBuf;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// Human-readable session name.
    pub name: String,
    /// Session kind (currently only "terminal" is supported).
    #[serde(rename = "type", default = "default_session_type")]
    pub session_type: String,
    /// Shell to spawn. Falls back to the configured default.
    pub shell: Option<String>,
    /// Working directory. Falls back to $CWD.
    pub cwd: Option<String>,
}

fn default_session_type() -> String {
    "terminal".to_string()
}

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub id: String,
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub shell: String,
    pub cwd: String,
}

#[derive(Serialize)]
pub struct DeleteSessionResponse {
    pub deleted: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/sessions — list all active PTY sessions.
pub async fn list_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let sessions = state.core.pty_manager.list_sessions();
    let list: Vec<SessionInfo> = sessions
        .into_iter()
        .map(|s| {
            let status = format!("{:?}", s.status);
            SessionInfo {
                id: s.id,
                name: s.name,
                status,
                created_at: s.created_at.to_rfc3339(),
                shell: s.shell,
                cwd: s.cwd.display().to_string(),
            }
        })
        .collect();
    Json(list)
}

/// POST /api/v1/sessions — create a new PTY session.
pub async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let shell = body.shell.as_deref();
    let cwd = body.cwd.as_ref().map(PathBuf::from);
    let cwd_ref = cwd.as_deref();

    match state
        .core
        .pty_manager
        .create_session(&body.name, shell, cwd_ref)
        .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(CreateSessionResponse { id }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/sessions/:id — kill and remove a PTY session.
pub async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.core.pty_manager.kill_session(&id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(DeleteSessionResponse { deleted: true }),
        )
            .into_response(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorBody { error: msg }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody { error: msg }),
                )
                    .into_response()
            }
        }
    }
}
