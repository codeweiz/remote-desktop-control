pub mod status;
pub mod terminal;

use axum::{http::StatusCode, response::IntoResponse};

pub use status::ws_status;
pub use terminal::ws_terminal;

/// Placeholder for the agent WebSocket upgrade handler.
///
/// Real implementation will be added in Task 21 (ACP Client).
pub async fn ws_agent_placeholder() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "WebSocket agent: not implemented yet")
}
