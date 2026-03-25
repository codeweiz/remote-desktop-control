use axum::{http::StatusCode, response::IntoResponse};

/// Fallback handler for static file requests.
///
/// This is a placeholder — the real implementation (Task 18) will serve
/// the embedded frontend SPA bundle via `rust-embed`.
pub async fn static_fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not found")
}
