use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use rtb_core::task_pool::types::*;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AddTaskRequest {
    /// Task title / name.
    pub title: String,
    /// Detailed prompt or description (defaults to title if omitted).
    #[serde(default)]
    pub prompt: String,
    /// Priority: "p0", "p1", or "p2".
    #[serde(default)]
    pub priority: Option<String>,
    /// Working directory for the task.
    #[serde(default)]
    pub cwd: Option<String>,
    /// IDs of tasks this task depends on.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Serialize)]
pub struct AddTaskResponse {
    pub id: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub priority: String,
    pub status: String,
    pub depends_on: Vec<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
struct MessageBody {
    message: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/v1/tasks — list all tasks.
pub async fn list_tasks(State(state): State<AppState>) -> impl IntoResponse {
    let tasks = state.core.task_pool.list(None).await;
    let infos: Vec<TaskInfo> = tasks
        .into_iter()
        .map(|t| TaskInfo {
            id: t.id,
            name: t.name,
            priority: t.priority.to_string(),
            status: t.status.to_string(),
            depends_on: t.depends_on,
            created_at: t.created_at.to_rfc3339(),
            started_at: t.started_at.map(|dt| dt.to_rfc3339()),
            completed_at: t.completed_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();
    Json(infos)
}

/// POST /api/v1/tasks — add a new task.
pub async fn add_task(
    State(state): State<AppState>,
    Json(body): Json<AddTaskRequest>,
) -> impl IntoResponse {
    let priority = match body.priority.as_deref() {
        Some("p0") | Some("P0") => Priority::P0,
        Some("p2") | Some("P2") => Priority::P2,
        _ => Priority::P1,
    };

    let prompt = if body.prompt.is_empty() {
        body.title.clone()
    } else {
        body.prompt.clone()
    };

    let mut task = Task::new(&body.title, prompt).with_priority(priority);

    if !body.depends_on.is_empty() {
        task = task.with_deps(body.depends_on);
    }

    if let Some(cwd) = body.cwd {
        task.target = TaskTarget::Agent {
            provider: String::new(),
            model: String::new(),
        };
        // Store cwd in tags for now (target doesn't have a cwd field for agent)
        task.tags.push(format!("cwd:{}", cwd));
    }

    match state.core.task_pool.add(task).await {
        Ok(id) => (
            StatusCode::CREATED,
            Json(AddTaskResponse {
                id,
                status: "queued".to_string(),
            }),
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

/// DELETE /api/v1/tasks/{id} — cancel a task.
pub async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Try to set status to Cancelled first (for queued/blocked/running tasks)
    if let Some(task) = state.core.task_pool.get(&id).await {
        if task.is_terminal() {
            // Already in terminal state, just remove it
            match state.core.task_pool.remove(&id).await {
                Ok(_) => {
                    return (
                        StatusCode::OK,
                        Json(MessageBody {
                            message: format!("Task {} removed", id),
                        }),
                    )
                        .into_response()
                }
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

        match state
            .core
            .task_pool
            .update_status(&id, TaskStatus::Cancelled)
            .await
        {
            Ok(()) => (
                StatusCode::OK,
                Json(MessageBody {
                    message: format!("Task {} cancelled", id),
                }),
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
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: format!("task not found: {}", id),
            }),
        )
            .into_response()
    }
}

/// POST /api/v1/tasks/{id}/approve — approve a completed task (needs_review -> completed).
pub async fn approve_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state
        .core
        .task_pool
        .update_status(&id, TaskStatus::Completed)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(MessageBody {
                message: format!("Task {} approved", id),
            }),
        )
            .into_response(),
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };
            (
                status,
                Json(ErrorBody {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// PATCH /api/v1/tasks/{id}
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    /// New priority: "p0", "p1", or "p2".
    #[serde(default)]
    pub priority: Option<String>,
    /// New position in the queue (0-based).
    #[serde(default)]
    pub position: Option<usize>,
}

/// PATCH /api/v1/tasks/{id} — update a task (change priority, reorder).
pub async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateTaskRequest>,
) -> impl IntoResponse {
    // Verify task exists
    let _task = match state.core.task_pool.get(&id).await {
        Some(t) => t,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    error: format!("task not found: {}", id),
                }),
            )
                .into_response()
        }
    };

    // Update priority if provided
    if let Some(ref priority_str) = body.priority {
        let new_priority = match priority_str.to_lowercase().as_str() {
            "p0" => Priority::P0,
            "p1" => Priority::P1,
            "p2" => Priority::P2,
            other => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorBody {
                        error: format!("invalid priority: {other}. Use p0, p1, or p2"),
                    }),
                )
                    .into_response()
            }
        };

        if let Err(e) = state
            .core
            .task_pool
            .update_priority(&id, new_priority)
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: e.to_string(),
                }),
            )
                .into_response();
        }
    }

    // Reorder if position is provided
    if let Some(position) = body.position {
        if let Err(e) = state.core.task_pool.reorder(&id, position).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: e.to_string(),
                }),
            )
                .into_response();
        }
    }

    // Return the updated task
    match state.core.task_pool.get(&id).await {
        Some(t) => Json(TaskInfo {
            id: t.id,
            name: t.name,
            priority: t.priority.to_string(),
            status: t.status.to_string(),
            depends_on: t.depends_on,
            created_at: t.created_at.to_rfc3339(),
            started_at: t.started_at.map(|dt| dt.to_rfc3339()),
            completed_at: t.completed_at.map(|dt| dt.to_rfc3339()),
        })
        .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: format!("task not found: {}", id),
            }),
        )
            .into_response(),
    }
}

/// POST /api/v1/tasks/scheduler/pause — pause the scheduler.
///
/// Note: The actual scheduler pause/resume requires a handle stored in AppState.
/// For now, this is a stub that returns success.
pub async fn pause_scheduler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(MessageBody {
            message: "Scheduler paused".to_string(),
        }),
    )
}

/// POST /api/v1/tasks/scheduler/resume — resume the scheduler.
pub async fn resume_scheduler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(MessageBody {
            message: "Scheduler resumed".to_string(),
        }),
    )
}
