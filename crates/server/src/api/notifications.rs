use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::state::AppState;

/// Query parameters for the notifications endpoint.
#[derive(Debug, Deserialize)]
pub struct NotificationsQuery {
    /// Only return notifications with id greater than this value.
    pub since_id: Option<u64>,
}

/// GET /api/v1/notifications — list recent notifications (up to 100).
pub async fn list_notifications(
    State(state): State<AppState>,
    Query(params): Query<NotificationsQuery>,
) -> impl IntoResponse {
    let notifications = match params.since_id {
        Some(id) => state.core.notification_store.list_since(id),
        None => state.core.notification_store.list(),
    };

    Json(notifications)
}
