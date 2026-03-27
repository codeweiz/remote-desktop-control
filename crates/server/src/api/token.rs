use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct RotateTokenRequest {
    pub new_token: String,
}

/// POST /api/v1/token/rotate
///
/// Rotates the in-memory authentication token. The caller must authenticate
/// with the current (old) token. After rotation, subsequent requests must
/// use the new token.
pub async fn rotate_token(
    State(state): State<AppState>,
    Json(body): Json<RotateTokenRequest>,
) -> impl IntoResponse {
    let new_token = body.new_token.trim().to_string();

    if new_token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "new_token must not be empty"
            })),
        );
    }

    // Update the in-memory token
    let mut token = state.token.write().await;
    let old_token = token.clone();
    *token = new_token.clone();
    drop(token);

    tracing::info!(
        old_prefix = &old_token[..old_token.len().min(8)],
        new_prefix = &new_token[..new_token.len().min(8)],
        "Authentication token rotated"
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "rotated",
            "message": "Token rotated successfully. Use the new token for subsequent requests."
        })),
    )
}
