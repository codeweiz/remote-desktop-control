use axum::{
    middleware,
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::Serialize;

use crate::{
    api::{notifications, plugins, sessions, status, tasks, token, tunnel},
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
        // Status
        .route("/status", get(status::get_status))
        // Sessions
        .route("/sessions", get(sessions::list_sessions))
        .route("/sessions", post(sessions::create_session))
        .route("/sessions/{id}", delete(sessions::delete_session))
        .route("/sessions/{id}/input", post(sessions::send_session_input))
        // Notifications
        .route("/notifications", get(notifications::list_notifications))
        // Token
        .route("/token/rotate", post(token::rotate_token))
        // Tasks
        .route("/tasks", get(tasks::list_tasks))
        .route("/tasks", post(tasks::add_task))
        .route("/tasks/{id}", delete(tasks::cancel_task))
        .route("/tasks/{id}", patch(tasks::update_task))
        .route("/tasks/{id}/approve", post(tasks::approve_task))
        .route("/tasks/scheduler/pause", post(tasks::pause_scheduler))
        .route("/tasks/scheduler/resume", post(tasks::resume_scheduler))
        // Plugins
        .route("/plugins", get(plugins::list_plugins))
        .route("/plugins/{name}/enable", post(plugins::enable_plugin))
        .route("/plugins/{name}/disable", post(plugins::disable_plugin))
        // Tunnel
        .route("/tunnel/status", get(tunnel::tunnel_status))
        .route("/tunnel/start", post(tunnel::start_tunnel))
        .route("/tunnel/stop", post(tunnel::stop_tunnel))
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
    let authed_api =
        Router::new()
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
