//! Task Scheduler — background task that watches for idle states
//! and automatically starts queued tasks.

use std::sync::Arc;

use tokio::sync::watch;
use tracing::{debug, error, info};

use super::pool::TaskPool;
use super::types::TaskStatus;

/// Scheduler configuration.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum concurrent tasks.
    pub max_concurrent: usize,
    /// Whether to automatically start tasks when idle.
    pub auto_start: bool,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 1,
            auto_start: true,
            poll_interval_secs: 5,
        }
    }
}

/// Background scheduler that monitors the task pool and starts tasks.
pub struct TaskScheduler {
    config: SchedulerConfig,
    pool: Arc<TaskPool>,
    /// Shutdown signal.
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl TaskScheduler {
    /// Create a new task scheduler.
    pub fn new(config: SchedulerConfig, pool: Arc<TaskPool>) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            config,
            pool,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Start the scheduler background loop.
    ///
    /// Returns a handle that can be used to stop the scheduler.
    pub fn start(&self) -> SchedulerHandle {
        let config = self.config.clone();
        let pool = Arc::clone(&self.pool);
        let mut shutdown_rx = self.shutdown_rx.clone();

        let handle = tokio::spawn(async move {
            info!(
                max_concurrent = config.max_concurrent,
                auto_start = config.auto_start,
                poll_interval_secs = config.poll_interval_secs,
                "task scheduler started"
            );

            let interval = tokio::time::Duration::from_secs(config.poll_interval_secs);

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        if config.auto_start {
                            Self::tick(&config, &pool).await;
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("task scheduler shutting down");
                            break;
                        }
                    }
                }
            }
        });

        SchedulerHandle {
            shutdown_tx: self.shutdown_tx.clone(),
            _join_handle: handle,
        }
    }

    /// One tick of the scheduler loop.
    async fn tick(config: &SchedulerConfig, pool: &TaskPool) {
        let running = pool.running_count().await;

        if running >= config.max_concurrent {
            debug!(
                running = running,
                max = config.max_concurrent,
                "at max concurrent, not starting new tasks"
            );
            return;
        }

        let slots = config.max_concurrent - running;

        for _ in 0..slots {
            match pool.get_next_executable().await {
                Some(task) => {
                    info!(
                        id = %task.id,
                        name = %task.name,
                        priority = %task.priority,
                        "scheduling task for execution"
                    );

                    // Mark as running
                    if let Err(e) = pool.update_status(&task.id, TaskStatus::Running).await {
                        error!(id = %task.id, error = %e, "failed to start task");
                        continue;
                    }

                    // Placeholder: In a real implementation, this would:
                    // 1. For Agent tasks: create an agent session via AgentManager
                    //    and send the task prompt as the initial message.
                    // 2. For Command tasks: create a PTY session and execute the command.
                    //
                    // The task result would be set when the agent/command completes,
                    // triggering update_status to Completed or Failed.
                    debug!(
                        id = %task.id,
                        target = ?task.target,
                        "task execution placeholder — actual execution will be connected to AgentManager/PtyManager"
                    );
                }
                None => {
                    debug!("no executable tasks in queue");
                    break;
                }
            }
        }
    }

    /// Get the current scheduler configuration.
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }
}

/// Handle to a running scheduler. Drop to stop the scheduler.
pub struct SchedulerHandle {
    shutdown_tx: watch::Sender<bool>,
    _join_handle: tokio::task::JoinHandle<()>,
}

impl SchedulerHandle {
    /// Signal the scheduler to stop.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
    }
}
