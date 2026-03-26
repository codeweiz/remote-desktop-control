//! Agent Manager — manages multiple ACP client sessions.
//!
//! Provides a centralized interface for creating, managing, and communicating
//! with agent subprocesses across multiple sessions.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::event_bus::EventBus;
use crate::events::{AgentStatus, ControlEvent, DataEvent, ErrorClass, SessionId};

use super::acp_client::{AcpClient, AcpError, AcpEvent};

/// Restart strategy for agents.
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    /// Maximum number of restart attempts.
    pub max_attempts: u32,
    /// Base backoff in seconds.
    pub backoff_base_secs: u64,
    /// Maximum backoff in seconds.
    pub backoff_max_secs: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_base_secs: 3,
            backoff_max_secs: 30,
        }
    }
}

/// Tracks state for a managed agent session.
struct ManagedAgent {
    client: AcpClient,
    /// Human-readable session name.
    name: String,
    /// When the agent was created.
    created_at: DateTime<Utc>,
    #[allow(dead_code)]
    agent_binary: String,
    restart_count: u32,
    restart_policy: RestartPolicy,
}

/// Manages the lifecycle of all agent sessions.
pub struct AgentManager {
    /// Active agents keyed by session ID.
    agents: Arc<DashMap<SessionId, ManagedAgent>>,
    /// Event bus for publishing control/data events.
    event_bus: Arc<EventBus>,
    /// Default restart policy.
    default_restart_policy: RestartPolicy,
}

impl AgentManager {
    /// Create a new agent manager.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            event_bus,
            default_restart_policy: RestartPolicy::default(),
        }
    }

    /// Create a new agent manager with a custom restart policy.
    pub fn with_restart_policy(event_bus: Arc<EventBus>, policy: RestartPolicy) -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            event_bus,
            default_restart_policy: policy,
        }
    }

    /// Create and start a new agent session.
    ///
    /// If the agent binary cannot be spawned or initialization fails, the agent
    /// is still registered in a "crashed" state so the caller can see it in the
    /// session list and understand what went wrong.
    pub async fn create_agent(
        &self,
        session_id: SessionId,
        name: &str,
        provider: &str,
        model: &str,
        cwd: PathBuf,
    ) -> Result<(), AcpError> {
        info!(
            session_id = %session_id,
            name = %name,
            provider = %provider,
            model = %model,
            cwd = %cwd.display(),
            "creating agent"
        );

        let mut client = AcpClient::new(
            session_id.clone(),
            provider.to_string(),
            model.to_string(),
            cwd,
        );

        let agent_binary = resolve_binary(provider);
        let created_at = Utc::now();

        // Attempt to start the agent. On failure, register it as crashed
        // so the session is still visible in the list.
        match client.start(&agent_binary).await {
            Ok(()) => {
                // Take the event receiver and start routing events
                if let Some(event_rx) = client.take_event_rx() {
                    self.start_event_router(session_id.clone(), event_rx);
                }

                let managed = ManagedAgent {
                    client,
                    name: name.to_string(),
                    created_at,
                    agent_binary,
                    restart_count: 0,
                    restart_policy: self.default_restart_policy.clone(),
                };

                self.agents.insert(session_id.clone(), managed);

                // Publish session creation event
                self.event_bus.publish_control(ControlEvent::AgentStatusChanged {
                    session_id,
                    status: AgentStatus::Ready,
                });

                Ok(())
            }
            Err(e) => {
                warn!(
                    session_id = %session_id,
                    error = %e,
                    "agent failed to start, registering in crashed state"
                );

                // Mark client as crashed
                client.status = AgentStatus::Crashed {
                    error: e.to_string(),
                    class: ErrorClass::Permanent,
                };

                let managed = ManagedAgent {
                    client,
                    name: name.to_string(),
                    created_at,
                    agent_binary,
                    restart_count: 0,
                    restart_policy: self.default_restart_policy.clone(),
                };

                self.agents.insert(session_id.clone(), managed);

                self.event_bus.publish_control(ControlEvent::AgentStatusChanged {
                    session_id: session_id.clone(),
                    status: AgentStatus::Crashed {
                        error: e.to_string(),
                        class: ErrorClass::Permanent,
                    },
                });

                Err(e)
            }
        }
    }

    /// Send a message to an agent.
    pub async fn send_message(
        &self,
        session_id: &str,
        text: String,
    ) -> Result<(), AcpError> {
        let agent = self
            .agents
            .get(session_id)
            .ok_or(AcpError::NotRunning)?;

        agent.client.send_message(text).await
    }

    /// Approve a tool use request.
    pub async fn approve_tool(
        &self,
        session_id: &str,
        tool_id: String,
    ) -> Result<(), AcpError> {
        let agent = self
            .agents
            .get(session_id)
            .ok_or(AcpError::NotRunning)?;

        agent.client.approve_tool(tool_id).await
    }

    /// Deny a tool use request.
    pub async fn deny_tool(
        &self,
        session_id: &str,
        tool_id: String,
        reason: Option<String>,
    ) -> Result<(), AcpError> {
        let agent = self
            .agents
            .get(session_id)
            .ok_or(AcpError::NotRunning)?;

        agent.client.deny_tool(tool_id, reason).await
    }

    /// Kill an agent session.
    pub async fn kill_agent(&self, session_id: &str) -> Result<(), AcpError> {
        if let Some(mut entry) = self.agents.get_mut(session_id) {
            entry.value_mut().client.kill().await;
            drop(entry);
            self.agents.remove(session_id);
            info!(session_id = %session_id, "agent killed");
            Ok(())
        } else {
            Err(AcpError::NotRunning)
        }
    }

    /// List all active agent sessions with metadata.
    pub fn list_agents(&self) -> Vec<(SessionId, String, AgentStatus, DateTime<Utc>)> {
        self.agents
            .iter()
            .map(|entry| {
                let m = entry.value();
                (
                    entry.key().clone(),
                    m.name.clone(),
                    m.client.status.clone(),
                    m.created_at,
                )
            })
            .collect()
    }

    /// Check if an agent session exists.
    pub fn has_agent(&self, session_id: &str) -> bool {
        self.agents.contains_key(session_id)
    }

    /// Get the number of active agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Start routing events from an agent's event channel to the EventBus.
    fn start_event_router(
        &self,
        session_id: SessionId,
        mut event_rx: mpsc::Receiver<AcpEvent>,
    ) {
        let event_bus = Arc::clone(&self.event_bus);
        let agents = Arc::clone(&self.agents);
        let sid = session_id.clone();

        tokio::spawn(async move {
            let mut seq: u64 = 1;

            while let Some(event) = event_rx.recv().await {
                match event {
                    AcpEvent::StatusChanged(status) => {
                        event_bus.publish_control(ControlEvent::AgentStatusChanged {
                            session_id: sid.clone(),
                            status,
                        });
                    }
                    AcpEvent::Content(content) => {
                        let data_event = DataEvent::AgentMessage {
                            seq,
                            content,
                        };
                        seq += 1;
                        event_bus.publish_data(&sid, data_event).await;
                    }
                    AcpEvent::ToolUseRequest { id: _, tool, input } => {
                        let data_event = DataEvent::AgentToolUse {
                            seq,
                            tool,
                            input,
                        };
                        seq += 1;
                        event_bus.publish_data(&sid, data_event).await;
                    }
                    AcpEvent::Error { message, class } => {
                        error!(
                            session_id = %sid,
                            error = %message,
                            class = ?class,
                            "agent error"
                        );
                        event_bus.publish_control(ControlEvent::AgentError {
                            session_id: sid.clone(),
                            error: message.clone(),
                            class: class.clone(),
                        });

                        // Handle restart for transient errors
                        if matches!(class, ErrorClass::Transient) {
                            Self::maybe_restart(&agents, &event_bus, &sid).await;
                        }
                    }
                    AcpEvent::Exited(code) => {
                        warn!(
                            session_id = %sid,
                            exit_code = ?code,
                            "agent process exited"
                        );
                        Self::maybe_restart(&agents, &event_bus, &sid).await;
                        break;
                    }
                }
            }
        });
    }

    /// Attempt to restart a crashed agent with backoff.
    async fn maybe_restart(
        agents: &Arc<DashMap<SessionId, ManagedAgent>>,
        event_bus: &EventBus,
        session_id: &str,
    ) {
        let should_restart = if let Some(mut entry) = agents.get_mut(session_id) {
            let managed = entry.value_mut();
            managed.restart_count += 1;

            if managed.restart_count > managed.restart_policy.max_attempts {
                error!(
                    session_id = %session_id,
                    attempts = managed.restart_count,
                    "agent exceeded max restart attempts, not restarting"
                );
                event_bus.publish_control(ControlEvent::AgentError {
                    session_id: session_id.to_string(),
                    error: "exceeded max restart attempts".to_string(),
                    class: ErrorClass::Permanent,
                });
                false
            } else {
                let backoff = std::cmp::min(
                    managed.restart_policy.backoff_base_secs
                        * 2u64.pow(managed.restart_count - 1),
                    managed.restart_policy.backoff_max_secs,
                );
                info!(
                    session_id = %session_id,
                    attempt = managed.restart_count,
                    backoff_secs = backoff,
                    "scheduling agent restart"
                );
                // In a real implementation, we'd use tokio::time::sleep(backoff)
                // then call client.start() again. For now, we just log.
                true
            }
        } else {
            false
        };

        if should_restart {
            debug!(session_id = %session_id, "agent restart scheduled");
        }
    }

    /// Shut down all agents.
    pub async fn shutdown_all(&self) {
        let keys: Vec<SessionId> = self.agents.iter().map(|e| e.key().clone()).collect();
        for session_id in keys {
            if let Some(mut entry) = self.agents.get_mut(&session_id) {
                entry.value_mut().client.kill().await;
            }
            self.agents.remove(&session_id);
        }
        info!("all agents shut down");
    }
}

/// Resolve an agent binary path from a provider name.
///
/// Maps well-known provider names to their CLI binary names.
/// Falls back to using the provider name directly.
fn resolve_binary(provider: &str) -> String {
    match provider {
        "claude-code" => "claude".to_string(),
        "gemini-cli" => "gemini".to_string(),
        "aider" => "aider".to_string(),
        "codex" => "codex".to_string(),
        other => other.to_string(),
    }
}
