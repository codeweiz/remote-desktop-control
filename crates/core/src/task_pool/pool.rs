//! Task Pool — in-memory task storage backed by tasks.jsonl.

use std::path::PathBuf;

use chrono::Utc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::types::*;

/// Errors from task pool operations.
#[derive(Debug, thiserror::Error)]
pub enum TaskPoolError {
    #[error("task not found: {0}")]
    NotFound(TaskId),
    #[error("invalid status transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// In-memory task pool with JSONL persistence.
pub struct TaskPool {
    /// All tasks, ordered by insertion.
    tasks: RwLock<Vec<Task>>,
    /// Path to the backing tasks.jsonl file.
    storage_path: PathBuf,
}

impl TaskPool {
    /// Create a new task pool with the given storage path.
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            tasks: RwLock::new(Vec::new()),
            storage_path,
        }
    }

    /// Load tasks from the JSONL file.
    pub async fn load(&self) -> Result<(), TaskPoolError> {
        if !self.storage_path.exists() {
            debug!(path = %self.storage_path.display(), "no tasks file found, starting fresh");
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.storage_path).await?;
        let mut tasks = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<Task>(line) {
                Ok(task) => tasks.push(task),
                Err(e) => {
                    warn!(
                        line = line_num + 1,
                        error = %e,
                        "failed to parse task line, skipping"
                    );
                }
            }
        }

        info!(count = tasks.len(), "loaded tasks from disk");
        *self.tasks.write().await = tasks;
        Ok(())
    }

    /// Save all tasks to the JSONL file.
    pub async fn save(&self) -> Result<(), TaskPoolError> {
        let tasks = self.tasks.read().await;
        let mut lines = String::new();

        for task in tasks.iter() {
            let json = serde_json::to_string(task)?;
            lines.push_str(&json);
            lines.push('\n');
        }

        // Ensure parent directory exists
        if let Some(parent) = self.storage_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.storage_path, lines).await?;
        debug!(path = %self.storage_path.display(), "saved tasks to disk");
        Ok(())
    }

    /// Add a new task to the pool. Returns the task ID.
    pub async fn add(&self, mut task: Task) -> Result<TaskId, TaskPoolError> {
        // Check if dependencies exist and update status to Blocked if needed
        {
            let tasks = self.tasks.read().await;
            let completed_ids: Vec<TaskId> = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Completed)
                .map(|t| t.id.clone())
                .collect();

            if !task.depends_on.is_empty() && !task.deps_satisfied(&completed_ids) {
                task.status = TaskStatus::Blocked;
            }
        }

        let id = task.id.clone();
        info!(id = %id, name = %task.name, priority = %task.priority, "adding task");

        self.tasks.write().await.push(task);
        self.save().await?;

        Ok(id)
    }

    /// Remove a task by ID.
    pub async fn remove(&self, id: &str) -> Result<Task, TaskPoolError> {
        let mut tasks = self.tasks.write().await;
        let pos = tasks
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| TaskPoolError::NotFound(id.to_string()))?;

        let task = tasks.remove(pos);
        drop(tasks);
        self.save().await?;

        info!(id = %id, "removed task");
        Ok(task)
    }

    /// Update the status of a task.
    pub async fn update_status(
        &self,
        id: &str,
        new_status: TaskStatus,
    ) -> Result<(), TaskPoolError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TaskPoolError::NotFound(id.to_string()))?;

        // Validate the transition
        if !is_valid_transition(&task.status, &new_status) {
            return Err(TaskPoolError::InvalidTransition {
                from: task.status.to_string(),
                to: new_status.to_string(),
            });
        }

        let now = Utc::now();
        task.updated_at = now;

        match new_status {
            TaskStatus::Running => {
                task.started_at = Some(now);
            }
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                task.completed_at = Some(now);
            }
            _ => {}
        }

        info!(id = %id, from = %task.status, to = %new_status, "updating task status");
        task.status = new_status;

        // After completing a task, check if any blocked tasks can be unblocked
        let completed_ids: Vec<TaskId> = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect();

        for t in tasks.iter_mut() {
            if t.status == TaskStatus::Blocked && t.deps_satisfied(&completed_ids) {
                t.status = TaskStatus::Queued;
                t.updated_at = now;
                debug!(id = %t.id, "unblocked task");
            }
        }

        drop(tasks);
        self.save().await?;

        Ok(())
    }

    /// Set the session ID for a running task.
    pub async fn set_session_id(&self, id: &str, session_id: String) -> Result<(), TaskPoolError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TaskPoolError::NotFound(id.to_string()))?;

        task.session_id = Some(session_id);
        task.updated_at = Utc::now();

        drop(tasks);
        self.save().await?;
        Ok(())
    }

    /// Find a running task by its session ID.
    pub async fn find_by_session_id(&self, session_id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks
            .iter()
            .find(|t| t.session_id.as_deref() == Some(session_id))
            .cloned()
    }

    /// Set the result for a completed/failed task.
    pub async fn set_result(&self, id: &str, result: TaskResult) -> Result<(), TaskPoolError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TaskPoolError::NotFound(id.to_string()))?;

        task.result = Some(result);
        task.updated_at = Utc::now();

        drop(tasks);
        self.save().await?;
        Ok(())
    }

    /// List all tasks, optionally filtered by status.
    pub async fn list(&self, status_filter: Option<&TaskStatus>) -> Vec<Task> {
        let tasks = self.tasks.read().await;
        match status_filter {
            Some(status) => tasks
                .iter()
                .filter(|t| t.status == *status)
                .cloned()
                .collect(),
            None => tasks.clone(),
        }
    }

    /// Get a task by ID.
    pub async fn get(&self, id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks.iter().find(|t| t.id == id).cloned()
    }

    /// Update the priority of a task.
    pub async fn update_priority(
        &self,
        id: &str,
        new_priority: Priority,
    ) -> Result<(), TaskPoolError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TaskPoolError::NotFound(id.to_string()))?;

        info!(id = %id, from = %task.priority, to = %new_priority, "updating task priority");
        task.priority = new_priority;
        task.updated_at = Utc::now();

        drop(tasks);
        self.save().await?;

        Ok(())
    }

    /// Reorder tasks by moving a task to a new position.
    pub async fn reorder(&self, id: &str, new_position: usize) -> Result<(), TaskPoolError> {
        let mut tasks = self.tasks.write().await;
        let pos = tasks
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| TaskPoolError::NotFound(id.to_string()))?;

        let task = tasks.remove(pos);
        let new_pos = new_position.min(tasks.len());
        tasks.insert(new_pos, task);

        drop(tasks);
        self.save().await?;

        debug!(id = %id, new_position = new_pos, "reordered task");
        Ok(())
    }

    /// Get the next executable task (priority FIFO, check deps).
    ///
    /// Returns the highest-priority task that is Queued and has all
    /// dependencies satisfied.
    pub async fn get_next_executable(&self) -> Option<Task> {
        let tasks = self.tasks.read().await;

        let completed_ids: Vec<TaskId> = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect();

        // Find all executable tasks
        let mut candidates: Vec<&Task> = tasks
            .iter()
            .filter(|t| t.is_executable(&completed_ids))
            .collect();

        // Sort by priority (P0 first), then by creation time (FIFO)
        candidates.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.created_at.cmp(&b.created_at))
        });

        candidates.first().cloned().cloned()
    }

    /// Get the count of tasks in each status.
    pub async fn status_counts(&self) -> std::collections::HashMap<String, usize> {
        let tasks = self.tasks.read().await;
        let mut counts = std::collections::HashMap::new();

        for task in tasks.iter() {
            *counts.entry(task.status.to_string()).or_insert(0) += 1;
        }

        counts
    }

    /// Get the number of currently running tasks.
    pub async fn running_count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Running)
            .count()
    }

    /// Get the total number of tasks.
    pub async fn total_count(&self) -> usize {
        self.tasks.read().await.len()
    }
}

/// Validate a status transition.
fn is_valid_transition(from: &TaskStatus, to: &TaskStatus) -> bool {
    matches!(
        (from, to),
        // Normal forward transitions
        (TaskStatus::Queued, TaskStatus::Running)
            | (TaskStatus::Queued, TaskStatus::Blocked)
            | (TaskStatus::Queued, TaskStatus::Cancelled)
            | (TaskStatus::Blocked, TaskStatus::Queued)
            | (TaskStatus::Blocked, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::Completed)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::NeedsReview)
            | (TaskStatus::Running, TaskStatus::Cancelled)
            | (TaskStatus::NeedsReview, TaskStatus::Completed)
            | (TaskStatus::NeedsReview, TaskStatus::Failed)
            | (TaskStatus::NeedsReview, TaskStatus::Cancelled)
            // Retry: failed can go back to queued
            | (TaskStatus::Failed, TaskStatus::Queued)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn make_pool() -> (TaskPool, TempDir) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");
        let pool = TaskPool::new(path);
        (pool, tmp)
    }

    #[tokio::test]
    async fn test_add_and_list() {
        let (pool, _tmp) = make_pool().await;

        let t1 = Task::new("Task 1", "Do something");
        let t2 = Task::new("Task 2", "Do something else");

        let id1 = pool.add(t1).await.unwrap();
        let id2 = pool.add(t2).await.unwrap();

        let tasks = pool.list(None).await;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, id1);
        assert_eq!(tasks[1].id, id2);
    }

    #[tokio::test]
    async fn test_remove() {
        let (pool, _tmp) = make_pool().await;

        let t1 = Task::new("Task 1", "Do something");
        let id = pool.add(t1).await.unwrap();

        assert_eq!(pool.total_count().await, 1);

        let removed = pool.remove(&id).await.unwrap();
        assert_eq!(removed.id, id);
        assert_eq!(pool.total_count().await, 0);
    }

    #[tokio::test]
    async fn test_remove_not_found() {
        let (pool, _tmp) = make_pool().await;
        let result = pool.remove("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let (pool, _tmp) = make_pool().await;

        let t_p2 = Task::new("Low priority", "P2 task").with_priority(Priority::P2);
        let t_p0 = Task::new("High priority", "P0 task").with_priority(Priority::P0);
        let t_p1 = Task::new("Normal priority", "P1 task").with_priority(Priority::P1);

        pool.add(t_p2).await.unwrap();
        pool.add(t_p0).await.unwrap();
        pool.add(t_p1).await.unwrap();

        let next = pool.get_next_executable().await.unwrap();
        assert_eq!(next.priority, Priority::P0);
        assert_eq!(next.name, "High priority");
    }

    #[tokio::test]
    async fn test_fifo_within_priority() {
        let (pool, _tmp) = make_pool().await;

        let t1 = Task::new("First", "First P1 task");
        let t2 = Task::new("Second", "Second P1 task");

        pool.add(t1).await.unwrap();
        // Slight delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        pool.add(t2).await.unwrap();

        let next = pool.get_next_executable().await.unwrap();
        assert_eq!(next.name, "First");
    }

    #[tokio::test]
    async fn test_dependency_blocking() {
        let (pool, _tmp) = make_pool().await;

        let t1 = Task::new("Parent", "Parent task");
        let parent_id = pool.add(t1).await.unwrap();

        let t2 = Task::new("Child", "Child task").with_deps(vec![parent_id.clone()]);
        let child_id = pool.add(t2).await.unwrap();

        // Child should be blocked
        let child = pool.get(&child_id).await.unwrap();
        assert_eq!(child.status, TaskStatus::Blocked);

        // Only parent should be executable
        let next = pool.get_next_executable().await.unwrap();
        assert_eq!(next.id, parent_id);

        // Complete the parent
        pool.update_status(&parent_id, TaskStatus::Running)
            .await
            .unwrap();
        pool.update_status(&parent_id, TaskStatus::Completed)
            .await
            .unwrap();

        // Now child should be unblocked and executable
        let child = pool.get(&child_id).await.unwrap();
        assert_eq!(child.status, TaskStatus::Queued);

        let next = pool.get_next_executable().await.unwrap();
        assert_eq!(next.id, child_id);
    }

    #[tokio::test]
    async fn test_status_transitions() {
        let (pool, _tmp) = make_pool().await;

        let t = Task::new("Test", "Test task");
        let id = pool.add(t).await.unwrap();

        // Valid: Queued -> Running
        pool.update_status(&id, TaskStatus::Running).await.unwrap();

        // Valid: Running -> Completed
        pool.update_status(&id, TaskStatus::Completed)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_invalid_transition() {
        let (pool, _tmp) = make_pool().await;

        let t = Task::new("Test", "Test task");
        let id = pool.add(t).await.unwrap();

        // Invalid: Queued -> Completed (must go through Running)
        let result = pool.update_status(&id, TaskStatus::Completed).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_persistence() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("tasks.jsonl");

        // Create and save
        {
            let pool = TaskPool::new(path.clone());
            let t = Task::new("Persistent", "Persisted task").with_priority(Priority::P0);
            pool.add(t).await.unwrap();
        }

        // Load and verify
        {
            let pool = TaskPool::new(path);
            pool.load().await.unwrap();
            let tasks = pool.list(None).await;
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].name, "Persistent");
            assert_eq!(tasks[0].priority, Priority::P0);
        }
    }

    #[tokio::test]
    async fn test_reorder() {
        let (pool, _tmp) = make_pool().await;

        let t1 = Task::new("First", "First task");
        let t2 = Task::new("Second", "Second task");
        let t3 = Task::new("Third", "Third task");

        let id1 = pool.add(t1).await.unwrap();
        pool.add(t2).await.unwrap();
        let id3 = pool.add(t3).await.unwrap();

        // Move third to first position
        pool.reorder(&id3, 0).await.unwrap();

        let tasks = pool.list(None).await;
        assert_eq!(tasks[0].id, id3);
        assert_eq!(tasks[1].id, id1);
    }

    #[tokio::test]
    async fn test_status_counts() {
        let (pool, _tmp) = make_pool().await;

        let t1 = Task::new("T1", "queued");
        let t2 = Task::new("T2", "will run");
        let t3 = Task::new("T3", "also queued");

        pool.add(t1).await.unwrap();
        let id2 = pool.add(t2).await.unwrap();
        pool.add(t3).await.unwrap();

        pool.update_status(&id2, TaskStatus::Running).await.unwrap();

        let counts = pool.status_counts().await;
        assert_eq!(counts.get("queued"), Some(&2));
        assert_eq!(counts.get("running"), Some(&1));
    }

    #[tokio::test]
    async fn test_no_executable_when_empty() {
        let (pool, _tmp) = make_pool().await;
        assert!(pool.get_next_executable().await.is_none());
    }

    #[tokio::test]
    async fn test_retry_failed_task() {
        let (pool, _tmp) = make_pool().await;

        let t = Task::new("Retry me", "retry task");
        let id = pool.add(t).await.unwrap();

        pool.update_status(&id, TaskStatus::Running).await.unwrap();
        pool.update_status(&id, TaskStatus::Failed).await.unwrap();

        // Failed -> Queued (retry)
        pool.update_status(&id, TaskStatus::Queued).await.unwrap();

        let task = pool.get(&id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Queued);
    }
}
