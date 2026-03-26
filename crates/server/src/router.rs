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
    logging::access_log_middleware,
    rate_limit::rate_limit_middleware,
    security::security_headers,
    state::AppState,
    static_files::static_handler,
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
/// - `/api/v1/*` — authenticated REST API (auth middleware)
/// - `/ws/*` — WebSocket endpoints (self-authenticated via query token)
/// - fallback — static file serving (placeholder)
///
/// Middleware applied (outermost first):
/// - Access logging (all requests)
/// - Security headers (all responses)
/// - Auth (API routes only; WS routes validate tokens themselves)
pub fn create_router(state: AppState) -> Router {
    // REST API routes that require auth middleware
    let authed_api = Router::new()
        .nest("/api/v1", api_routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // WebSocket routes handle their own token validation via query params,
    // so they bypass the auth middleware (which would redirect on token= queries).
    let ws_routes = Router::new()
        .route("/ws/terminal", get(ws::ws_terminal))
        .route("/ws/agent", get(ws::ws_agent))
        .route("/ws/status", get(ws::ws_status));

    Router::new()
        // Public routes
        .route("/health", get(health))
        // Authenticated REST API
        .merge(authed_api)
        // WebSocket routes (self-authenticated)
        .merge(ws_routes)
        // Fallback: embedded frontend static files (SPA)
        .fallback(static_handler)
        // Security headers on every response
        .layer(middleware::from_fn(security_headers))
        // Per-IP rate limiting + blocklist check (before auth)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        // Access logging on every request
        .layer(middleware::from_fn(access_log_middleware))
        .with_state(state)
}
