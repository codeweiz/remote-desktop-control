//! Agent Manager — manages multiple ACP-backed agent sessions.
//!
//! Provides a centralized interface for creating, managing, and communicating
//! with agent subprocesses across multiple sessions.  Uses `AcpBackend` as the
//! single execution backend for all agent kinds.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tracing::{info, warn};

use crate::event_bus::EventBus;
use crate::events::{AgentStatus, ControlEvent, DataEvent, ErrorClass, SessionId};

use super::acp_backend::AcpBackend;
use super::event::{AgentEvent, AgentKind};

/// Tracks state for a managed agent session.
struct ManagedAgent {
    backend: AcpBackend,
    /// Human-readable session name.
    name: String,
    /// The kind of agent (Claude, Gemini, etc.).
    #[allow(dead_code)]
    kind: AgentKind,
    /// Working directory the agent was started in.
    #[allow(dead_code)]
    cwd: PathBuf,
    /// When the agent was created.
    created_at: DateTime<Utc>,
    /// How many times this agent has been restarted.
    #[allow(dead_code)]
    restart_count: u32,
    /// Optional companion terminal session for this agent.
    #[allow(dead_code)]
    companion_terminal_id: Option<String>,
}

/// Manages the lifecycle of all agent sessions.
pub struct AgentManager {
    /// Active agents keyed by session ID.
    agents: DashMap<SessionId, ManagedAgent>,
    /// Event bus for publishing control/data events.
    event_bus: Arc<EventBus>,
}

impl AgentManager {
    /// Create a new agent manager.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            agents: DashMap::new(),
            event_bus,
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
        _model: &str,
        cwd: PathBuf,
    ) -> Result<(), String> {
        info!(
            session_id = %session_id,
            name = %name,
            provider = %provider,
            cwd = %cwd.display(),
            "creating agent"
        );

        let kind = parse_agent_kind(provider);
        let mut backend = AcpBackend::new(kind.clone());
        let created_at = Utc::now();

        // Attempt to start the backend. On failure, register in crashed state.
        match backend.start(&cwd, None).await {
            Ok(()) => {
                // Start event router that bridges backend events to the event bus.
                self.start_event_router(session_id.clone(), &backend);

                let managed = ManagedAgent {
                    backend,
                    name: name.to_string(),
                    kind,
                    cwd,
                    created_at,
                    restart_count: 0,
                    companion_terminal_id: None,
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

                let managed = ManagedAgent {
                    backend,
                    name: name.to_string(),
                    kind,
                    cwd,
                    created_at,
                    restart_count: 0,
                    companion_terminal_id: None,
                };

                self.agents.insert(session_id.clone(), managed);

                self.event_bus.publish_control(ControlEvent::AgentStatusChanged {
                    session_id: session_id.clone(),
                    status: AgentStatus::Crashed {
                        error: e.clone(),
                        class: ErrorClass::Permanent,
                    },
                });

                Err(e)
            }
        }
    }

    /// Send a message to an agent (fire-and-forget).
    ///
    /// The caller uses the event stream to detect when the turn finishes.
    pub async fn send_message(
        &self,
        session_id: &str,
        text: String,
    ) -> Result<(), String> {
        let agent = self
            .agents
            .get(session_id)
            .ok_or_else(|| "Agent not running".to_string())?;

        agent.backend.send_message_fire(&text).await
    }

    /// Kill an agent session.
    pub async fn kill_agent(&self, session_id: &str) -> Result<(), String> {
        if let Some(mut entry) = self.agents.get_mut(session_id) {
            entry.value_mut().backend.shutdown().await;
            drop(entry);
            self.agents.remove(session_id);
            info!(session_id = %session_id, "agent killed");
            Ok(())
        } else {
            Err("Agent not running".to_string())
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
                    AgentStatus::Ready, // Simplified: actual status tracked via events
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

    /// Start routing events from the AcpBackend's broadcast channel to the EventBus.
    fn start_event_router(&self, session_id: String, backend: &AcpBackend) {
        let mut rx = backend.subscribe();
        let event_bus = self.event_bus.clone();
        let sid = session_id.clone();
        let mut seq: u64 = 1;

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                let data_event = match event {
                    AgentEvent::Text(content) => DataEvent::AgentText {
                        seq,
                        content,
                        streaming: true,
                    },
                    AgentEvent::Thinking(content) => DataEvent::AgentThinking { seq, content },
                    AgentEvent::ToolUse { name, id, input } => DataEvent::AgentToolUse {
                        seq,
                        id,
                        name,
                        input: serde_json::Value::String(input.unwrap_or_default()),
                    },
                    AgentEvent::ToolResult {
                        id,
                        output,
                        is_error,
                    } => DataEvent::AgentToolResult {
                        seq,
                        id,
                        output: output.unwrap_or_default(),
                        is_error,
                    },
                    AgentEvent::Progress(message) => DataEvent::AgentProgress { seq, message },
                    AgentEvent::TurnComplete { cost_usd, .. } => {
                        DataEvent::AgentTurnComplete { seq, cost_usd }
                    }
                    AgentEvent::Error(message) => DataEvent::AgentError {
                        seq,
                        message,
                        severity: ErrorClass::Transient,
                        guidance: String::new(),
                    },
                };
                seq += 1;
                event_bus.publish_data(&sid, data_event).await;
            }
        });
    }

    /// Shut down all agents.
    pub async fn shutdown_all(&self) {
        let keys: Vec<SessionId> = self.agents.iter().map(|e| e.key().clone()).collect();
        for session_id in keys {
            if let Some(mut entry) = self.agents.get_mut(&session_id) {
                entry.value_mut().backend.shutdown().await;
            }
            self.agents.remove(&session_id);
        }
        info!("all agents shut down");
    }
}

/// Parse a provider string into an AgentKind.
fn parse_agent_kind(provider: &str) -> AgentKind {
    match provider {
        "claude" | "claude-code" => AgentKind::Claude,
        "gemini" | "gemini-cli" => AgentKind::Gemini,
        "opencode" => AgentKind::OpenCode,
        "codex" => AgentKind::Codex,
        // Default to Claude for unknown providers
        _ => AgentKind::Claude,
    }
}
