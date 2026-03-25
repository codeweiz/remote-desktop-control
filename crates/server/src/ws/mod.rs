use axum::{http::StatusCode, response::IntoResponse};

/// Placeholder for the terminal WebSocket upgrade handler.
///
/// Real implementation will be added in Task 9 (WebSocket Handlers).
pub async fn ws_terminal_placeholder() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "WebSocket terminal: not implemented yet")
}

/// Placeholder for the agent WebSocket upgrade handler.
pub async fn ws_agent_placeholder() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "WebSocket agent: not implemented yet")
}

/// Placeholder for the status WebSocket upgrade handler.
pub async fn ws_status_placeholder() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "WebSocket status: not implemented yet")
}
