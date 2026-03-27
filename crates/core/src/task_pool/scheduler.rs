//! Task Dispatcher — background task that watches for idle agents
//! and automatically dispatches pending tasks from the pool.
//!
//! The dispatcher periodically checks for available capacity (idle agents
//! and concurrency limits) and assigns the highest-priority pending task
//! to an idle agent session. It also listens for `AgentTurnComplete`
//! events to transition tasks to Completed or NeedsReview.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use crate::agent::manager::AgentManager;
use crate::config::TaskPoolConfig;
use crate::event_bus::EventBus;
use crate::events::{AgentStatus, ControlEvent};

use super::pool::TaskPool;
use super::types::{TaskResult, TaskStatus, TaskTarget};

/// Scheduler configuration.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum concurrent tasks.
    pub max_concurrent: usize,
    /// Whether to automatically start tasks when idle.
    pub auto_start: bool,
    /// Whether completed tasks skip review (auto-approve).
    pub auto_approve: bool,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
    /// Default working directory for agent tasks.
    pub default_cwd: PathBuf,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 1,
            auto_start: true,
            auto_approve: false,
            poll_interval_secs: 5,
            default_cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        }
    }
}

impl SchedulerConfig {
    /// Build a SchedulerConfig from the application TaskPoolConfig.
    pub fn from_pool_config(cfg: &TaskPoolConfig) -> Self {
        Self {
            max_concurrent: cfg.max_concurrent,
            auto_start: cfg.auto_start,
            auto_approve: cfg.auto_approve,
            poll_interval_secs: 5,
            default_cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        }
    }
}

/// Background dispatcher that monitors the task pool and assigns tasks to agents.
///
/// Combines the previous `TaskScheduler` behaviour with actual agent dispatch:
/// - Polls for idle capacity on a configurable interval.
/// - Creates agent sessions via `AgentManager` for pending tasks.
/// - Subscribes to the `EventBus` control channel to detect turn completions.
pub struct TaskDispatcher {
    config: SchedulerConfig,
    pool: Arc<TaskPool>,
    agent_manager: Arc<AgentManager>,
    event_bus: Arc<EventBus>,
    /// Shutdown signal.
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl TaskDispatcher {
    /// Create a new task dispatcher.
    pub fn new(
        config: SchedulerConfig,
        pool: Arc<TaskPool>,
        agent_manager: Arc<AgentManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            config,
            pool,
            agent_manager,
            event_bus,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Start the dispatcher background loop.
    ///
    /// Returns a handle that can be used to stop the dispatcher.
    /// The dispatcher runs two concurrent tasks:
    /// 1. A polling loop that checks for idle capacity and dispatches tasks.
    /// 2. An event listener that watches for agent turn completions.
    pub fn start(&self) -> DispatcherHandle {
        let config = self.config.clone();
        let pool = Arc::clone(&self.pool);
        let agent_manager = Arc::clone(&self.agent_manager);
        let event_bus = Arc::clone(&self.event_bus);
        let mut shutdown_rx = self.shutdown_rx.clone();

        let handle = tokio::spawn(async move {
            info!(
                max_concurrent = config.max_concurrent,
                auto_start = config.auto_start,
                auto_approve = config.auto_approve,
                poll_interval_secs = config.poll_interval_secs,
                "task dispatcher started"
            );

            let interval = tokio::time::Duration::from_secs(config.poll_interval_secs);

            // Subscribe to control events for turn-complete detection.
            let mut control_rx = event_bus.subscribe_control();

            loop {
                tokio::select! {
                    // --- Periodic polling for pending tasks ---
                    _ = tokio::time::sleep(interval) => {
                        if config.auto_start {
                            Self::dispatch_tick(&config, &pool, &agent_manager, &event_bus).await;
                        }
                    }
                    // --- Event-driven: agent status changes ---
                    result = control_rx.recv() => {
                        match result {
                            Ok(event) => {
                                Self::handle_control_event(&config, &pool, &event).await;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!(skipped = n, "dispatcher lagged on control events");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                info!("control channel closed, dispatcher stopping");
                                break;
                            }
                        }
                    }
                    // --- Shutdown signal ---
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("task dispatcher shutting down");
                            break;
                        }
                    }
                }
            }
        });

        DispatcherHandle {
            shutdown_tx: self.shutdown_tx.clone(),
            _join_handle: handle,
        }
    }

    /// One tick of the dispatch loop: find pending tasks and assign to idle agents.
    async fn dispatch_tick(
        config: &SchedulerConfig,
        pool: &TaskPool,
        agent_manager: &AgentManager,
        event_bus: &EventBus,
    ) {
        let running = pool.running_count().await;

        if running >= config.max_concurrent {
            debug!(
                running = running,
                max = config.max_concurrent,
                "at max concurrent tasks, skipping dispatch"
            );
            return;
        }

        let slots = config.max_concurrent - running;

        for _ in 0..slots {
            let task = match pool.get_next_executable().await {
                Some(task) => task,
                None => {
                    debug!("no executable tasks in queue");
                    break;
                }
            };

            info!(
                id = %task.id,
                name = %task.name,
                priority = %task.priority,
                "dispatching task to agent"
            );

            // Determine provider/model from task target or use defaults.
            let (provider, model) = match &task.target {
                TaskTarget::Agent { provider, model } => {
                    let p = if provider.is_empty() {
                        "claude-code"
                    } else {
                        provider.as_str()
                    };
                    let m = if model.is_empty() { "" } else { model.as_str() };
                    (p.to_string(), m.to_string())
                }
                TaskTarget::Command { .. } => {
                    // For command tasks, we still route through an agent session
                    // that can execute the command. Future: direct PTY execution.
                    ("claude-code".to_string(), String::new())
                }
            };

            // Generate a unique session ID for this task's agent.
            let session_id = format!("task-{}", &task.id);

            // Build the system prompt injection.
            let system_prompt = build_task_prompt(&task);

            // Mark as running first — if agent creation fails we'll revert.
            if let Err(e) = pool.update_status(&task.id, TaskStatus::Running).await {
                error!(id = %task.id, error = %e, "failed to mark task as running");
                continue;
            }

            // Create an agent session for this task.
            let cwd = match &task.target {
                TaskTarget::Command { cwd, .. } => cwd
                    .as_ref()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| config.default_cwd.clone()),
                _ => config.default_cwd.clone(),
            };

            match agent_manager
                .create_agent(
                    session_id.clone(),
                    &format!("task:{}", task.name),
                    &provider,
                    &model,
                    cwd,
                )
                .await
            {
                Ok(()) => {
                    // Link task to its agent session.
                    if let Err(e) = pool.set_session_id(&task.id, session_id.clone()).await {
                        error!(id = %task.id, error = %e, "failed to set session_id on task");
                    }

                    // Send the task prompt to the agent.
                    if let Err(e) = agent_manager.send_message(&session_id, system_prompt).await {
                        error!(
                            id = %task.id,
                            session_id = %session_id,
                            error = %e,
                            "failed to send task prompt to agent"
                        );
                        // Mark task as failed since agent couldn't accept prompt.
                        let _ = pool.update_status(&task.id, TaskStatus::Failed).await;
                        let _ = pool
                            .set_result(
                                &task.id,
                                TaskResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("Failed to send prompt: {}", e)),
                                    exit_code: None,
                                    duration_secs: 0.0,
                                },
                            )
                            .await;
                        continue;
                    }

                    info!(
                        id = %task.id,
                        session_id = %session_id,
                        "task dispatched to agent successfully"
                    );

                    // Publish a control event so the UI knows.
                    event_bus.publish_control(ControlEvent::AgentStatusChanged {
                        session_id,
                        status: AgentStatus::Working,
                    });
                }
                Err(e) => {
                    error!(
                        id = %task.id,
                        error = %e,
                        "failed to create agent for task, marking as failed"
                    );
                    // Revert to failed since we couldn't spawn an agent.
                    let _ = pool.update_status(&task.id, TaskStatus::Failed).await;
                    let _ = pool
                        .set_result(
                            &task.id,
                            TaskResult {
                                success: false,
                                output: None,
                                error: Some(format!("Agent creation failed: {}", e)),
                                exit_code: None,
                                duration_secs: 0.0,
                            },
                        )
                        .await;
                }
            }
        }
    }

    /// Handle a control event — look for agent status changes that indicate
    /// a task's agent turn has completed (or crashed).
    async fn handle_control_event(config: &SchedulerConfig, pool: &TaskPool, event: &ControlEvent) {
        if let ControlEvent::AgentStatusChanged { session_id, status } = event {
            // Only care about task-dispatched sessions (prefixed "task-").
            if !session_id.starts_with("task-") {
                return;
            }

            match status {
                AgentStatus::Idle => {
                    // Agent finished its turn. Transition task based on auto_approve.
                    Self::handle_agent_idle(config, pool, session_id).await;
                }
                AgentStatus::Crashed { error, .. } => {
                    // Agent crashed — fail the task.
                    Self::handle_agent_crash(pool, session_id, error).await;
                }
                _ => {}
            }
        }
    }

    /// Agent went idle — the turn completed. Transition the task.
    async fn handle_agent_idle(config: &SchedulerConfig, pool: &TaskPool, session_id: &str) {
        if let Some(task) = pool.find_by_session_id(session_id).await {
            if task.status != TaskStatus::Running {
                return;
            }

            let new_status = if config.auto_approve {
                TaskStatus::Completed
            } else {
                TaskStatus::NeedsReview
            };

            info!(
                id = %task.id,
                session_id = %session_id,
                new_status = %new_status,
                auto_approve = config.auto_approve,
                "agent turn complete, transitioning task"
            );

            if let Err(e) = pool.update_status(&task.id, new_status.clone()).await {
                error!(id = %task.id, error = %e, "failed to transition task after agent idle");
                return;
            }

            if new_status == TaskStatus::Completed {
                let started = task.started_at.unwrap_or(task.created_at);
                let duration = chrono::Utc::now()
                    .signed_duration_since(started)
                    .num_seconds() as f64;

                let _ = pool
                    .set_result(
                        &task.id,
                        TaskResult {
                            success: true,
                            output: Some("Agent completed the task.".to_string()),
                            error: None,
                            exit_code: None,
                            duration_secs: duration,
                        },
                    )
                    .await;
            }
        }
    }

    /// Agent crashed — fail the associated task.
    async fn handle_agent_crash(pool: &TaskPool, session_id: &str, error: &str) {
        if let Some(task) = pool.find_by_session_id(session_id).await {
            if task.status != TaskStatus::Running {
                return;
            }

            warn!(
                id = %task.id,
                session_id = %session_id,
                error = %error,
                "agent crashed, failing task"
            );

            let started = task.started_at.unwrap_or(task.created_at);
            let duration = chrono::Utc::now()
                .signed_duration_since(started)
                .num_seconds() as f64;

            let _ = pool.update_status(&task.id, TaskStatus::Failed).await;
            let _ = pool
                .set_result(
                    &task.id,
                    TaskResult {
                        success: false,
                        output: None,
                        error: Some(format!("Agent crashed: {}", error)),
                        exit_code: None,
                        duration_secs: duration,
                    },
                )
                .await;
        }
    }

    /// Get the current dispatcher configuration.
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }
}

/// Build the system prompt injected into the agent for a task.
fn build_task_prompt(task: &super::types::Task) -> String {
    let priority = &task.priority;
    let description = &task.prompt;
    let name = &task.name;

    let mut prompt = format!(
        "You are working on a task from the task pool:\n\
         Title: {name}\n\
         Priority: {priority}\n\
         Description: {description}\n\
         \n\
         Please complete this task in the current working directory. When done, explain what you did."
    );

    // If this is a command task, include the command.
    if let TaskTarget::Command { command, cwd } = &task.target {
        prompt.push_str(&format!(
            "\n\nThis is a command task. Please execute the following command:\n```\n{command}\n```"
        ));
        if let Some(cwd) = cwd {
            prompt.push_str(&format!("\nWorking directory: {cwd}"));
        }
    }

    prompt
}

/// Handle to a running dispatcher. Drop to stop the dispatcher.
pub struct DispatcherHandle {
    shutdown_tx: watch::Sender<bool>,
    _join_handle: tokio::task::JoinHandle<()>,
}

impl DispatcherHandle {
    /// Signal the dispatcher to stop.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

impl Drop for DispatcherHandle {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
    }
}

// Re-export the old names for backward compatibility.
pub type TaskScheduler = TaskDispatcher;
pub type SchedulerHandle = DispatcherHandle;
