use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct StatusResponse {
    pub version: &'static str,
    pub uptime_secs: u64,
    pub sessions: usize,
}

/// GET /api/v1/status — server status snapshot.
pub async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    let sessions = state.core.pty_manager.session_count();

    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: 0, // TODO: track actual uptime via Instant in AppState
        sessions,
    })
}
