//! Task Pool types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique task identifier.
pub type TaskId = String;

/// Task priority levels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Highest priority — execute immediately when possible.
    P0 = 0,
    /// Normal priority.
    #[default]
    P1 = 1,
    /// Low priority — background tasks.
    P2 = 2,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::P0 => write!(f, "P0"),
            Priority::P1 => write!(f, "P1"),
            Priority::P2 => write!(f, "P2"),
        }
    }
}

/// Task execution target.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTarget {
    /// Execute via an AI agent session.
    Agent {
        /// Agent provider to use.
        #[serde(default)]
        provider: String,
        /// Model to use.
        #[serde(default)]
        model: String,
    },
    /// Execute as a shell command.
    Command {
        /// The command to run.
        command: String,
        /// Working directory.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
}

impl Default for TaskTarget {
    fn default() -> Self {
        TaskTarget::Agent {
            provider: String::new(),
            model: String::new(),
        }
    }
}

/// Task execution status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting in the queue.
    #[default]
    Queued,
    /// Blocked by unfinished dependencies.
    Blocked,
    /// Currently executing.
    Running,
    /// Completed but needs human review.
    NeedsReview,
    /// Successfully completed.
    Completed,
    /// Failed with an error.
    Failed,
    /// Cancelled by the user.
    Cancelled,
}

// TaskStatus uses #[derive(Default)] with #[default] on Queued variant.

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Queued => write!(f, "queued"),
            TaskStatus::Blocked => write!(f, "blocked"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::NeedsReview => write!(f, "needs_review"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Result of a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Whether the task succeeded.
    pub success: bool,
    /// Output or summary text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Error message if failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Exit code (for command tasks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Duration in seconds.
    #[serde(default)]
    pub duration_secs: f64,
}

/// A task in the task pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier.
    pub id: TaskId,
    /// Human-readable task name.
    pub name: String,
    /// Detailed prompt or description.
    pub prompt: String,
    /// Priority level.
    #[serde(default)]
    pub priority: Priority,
    /// Current status.
    #[serde(default)]
    pub status: TaskStatus,
    /// Execution target.
    #[serde(default)]
    pub target: TaskTarget,
    /// IDs of tasks that must complete before this one can start.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskId>,
    /// Optional tags for filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task was last updated.
    pub updated_at: DateTime<Utc>,
    /// When the task started executing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    /// When the task completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Session ID if the task is currently running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Task result (set when completed or failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResult>,
}

impl Task {
    /// Create a new task with the given name and prompt.
    pub fn new(name: impl Into<String>, prompt: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: nanoid::nanoid!(12),
            name: name.into(),
            prompt: prompt.into(),
            priority: Priority::P1,
            status: TaskStatus::Queued,
            target: TaskTarget::default(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            session_id: None,
            result: None,
        }
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the execution target.
    pub fn with_target(mut self, target: TaskTarget) -> Self {
        self.target = target;
        self
    }

    /// Add dependencies.
    pub fn with_deps(mut self, deps: Vec<TaskId>) -> Self {
        self.depends_on = deps;
        self
    }

    /// Add tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Check if all dependencies are satisfied.
    pub fn deps_satisfied(&self, completed_ids: &[TaskId]) -> bool {
        self.depends_on.iter().all(|dep| completed_ids.contains(dep))
    }

    /// Whether this task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// Whether this task can be started right now.
    pub fn is_executable(&self, completed_ids: &[TaskId]) -> bool {
        self.status == TaskStatus::Queued && self.deps_satisfied(completed_ids)
    }
}
