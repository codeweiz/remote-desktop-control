use axum::{
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;

use crate::{
    api::{sessions, status},
    auth::auth_middleware,
    security::security_headers,
    state::AppState,
    static_files::static_fallback,
    ws,
};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Health-check handler (unauthenticated).
async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

/// Build the set of API routes that require authentication.
fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/status", get(status::get_status))
        .route("/sessions", get(sessions::list_sessions))
        .route("/sessions", post(sessions::create_session))
        .route("/sessions/{id}", delete(sessions::delete_session))
}

/// Assemble the complete application router.
///
/// Layout:
/// - `/health` — public health check
/// - `/api/v1/*` — authenticated REST API
/// - `/ws/*` — WebSocket endpoints (placeholders)
/// - fallback — static file serving (placeholder)
///
/// Middleware applied (outermost first):
/// - Security headers (all responses)
/// - Auth (API + WS routes only)
pub fn create_router(state: AppState) -> Router {
    // Routes that require authentication
    let authed = Router::new()
        .nest("/api/v1", api_routes())
        .route("/ws/terminal", get(ws::ws_terminal_placeholder))
        .route("/ws/agent", get(ws::ws_agent_placeholder))
        .route("/ws/status", get(ws::ws_status_placeholder))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        // Public routes
        .route("/health", get(health))
        // Authenticated routes
        .merge(authed)
        // Fallback: static files (placeholder)
        .fallback(static_fallback)
        // Security headers on every response
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}
