use std::path::PathBuf;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use rtb_core::events::AgentStatus;
use rtb_core::pty::session::PtyStatus;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// Human-readable session name.
    pub name: String,
    /// Session kind: "terminal" or "agent".
    #[serde(rename = "type", default = "default_session_type")]
    pub session_type: String,
    /// Shell to spawn. Falls back to the configured default.
    pub shell: Option<String>,
    /// Working directory. Falls back to $CWD.
    pub cwd: Option<String>,
    /// Agent provider (e.g., "claude-code"). Only for agent sessions.
    pub provider: Option<String>,
    /// Agent model (e.g., "claude-sonnet-4-20250514"). Only for agent sessions.
    pub model: Option<String>,
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
    pub kind: String,
    pub status: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub exit_code: Option<i32>,
    pub shell: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/sessions — list all active sessions (terminal + agent).
pub async fn list_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let mut list: Vec<SessionInfo> = Vec::new();

    // Terminal sessions
    let sessions = state.core.pty_manager.list_sessions();
    for s in sessions {
        let (status, exit_code) = match s.status {
            PtyStatus::Running => ("running".to_string(), None),
            PtyStatus::Exited(code) => ("exited".to_string(), Some(code)),
        };
        list.push(SessionInfo {
            id: s.id,
            name: s.name,
            kind: "terminal".to_string(),
            status,
            parent_id: None,
            created_at: s.created_at.to_rfc3339(),
            exit_code,
            shell: Some(s.shell),
            cols: 80,
            rows: 24,
        });
    }

    // Agent sessions
    let agents = state.core.agent_manager.list_agents();
    for (agent_id, agent_status) in agents {
        let status = match agent_status {
            AgentStatus::Initializing => "initializing",
            AgentStatus::Ready => "ready",
            AgentStatus::Working => "working",
            AgentStatus::WaitingApproval => "waiting_approval",
            AgentStatus::Idle => "idle",
            AgentStatus::Crashed { .. } => "crashed",
        };
        list.push(SessionInfo {
            id: agent_id,
            name: String::new(),
            kind: "agent".to_string(),
            status: status.to_string(),
            parent_id: None,
            created_at: String::new(),
            exit_code: None,
            shell: None,
            cols: 0,
            rows: 0,
        });
    }

    Json(list)
}

/// POST /api/v1/sessions — create a new session (terminal or agent).
pub async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    match body.session_type.as_str() {
        "agent" => {
            let provider = body.provider.as_deref().unwrap_or("claude-code");
            let model = body.model.as_deref().unwrap_or("");
            let cwd = body
                .cwd
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
            let session_id = nanoid::nanoid!(10);

            match state
                .core
                .agent_manager
                .create_agent(session_id.clone(), &body.name, provider, model, cwd)
                .await
            {
                Ok(()) => (
                    StatusCode::CREATED,
                    Json(CreateSessionResponse { id: session_id }),
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
        _ => {
            // Default: terminal session
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
    }
}

/// DELETE /api/v1/sessions/:id — kill and remove a session (terminal or agent).
pub async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Try agent first, then fall back to PTY
    if state.core.agent_manager.has_agent(&id) {
        match state.core.agent_manager.kill_agent(&id).await {
            Ok(()) => return StatusCode::NO_CONTENT.into_response(),
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }

    match state.core.pty_manager.kill_session(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
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
