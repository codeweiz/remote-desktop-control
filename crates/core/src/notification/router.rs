//! Notification Router — routes detection triggers to configured channels.
//!
//! Currently emits notifications via the EventBus. Actual delivery to
//! web/desktop/IM channels is handled by the server and IM bridge layers.

use std::sync::Arc;

use tracing::{debug, info};

use crate::event_bus::EventBus;
use crate::events::{AgentStatus, ControlEvent, ErrorClass, SessionId};

use super::NotifyTrigger;

/// Configuration for the notification router.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Enabled notification channels.
    pub channels: Vec<String>,
    /// Whether sound notifications are enabled.
    pub sound_enabled: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            channels: vec!["web".to_string(), "desktop".to_string()],
            sound_enabled: false,
        }
    }
}

/// Routes notification triggers to configured channels.
pub struct NotificationRouter {
    config: RouterConfig,
    event_bus: Arc<EventBus>,
}

impl NotificationRouter {
    /// Create a new notification router.
    pub fn new(config: RouterConfig, event_bus: Arc<EventBus>) -> Self {
        Self { config, event_bus }
    }

    /// Route a batch of triggers to appropriate channels.
    pub fn route(&self, session_id: &SessionId, triggers: &[NotifyTrigger]) {
        for trigger in triggers {
            self.route_single(session_id, trigger);
        }
    }

    /// Route a single trigger — emits a NotificationTriggered control event for
    /// every trigger type so that WS clients and IM bridge can pick them up.
    fn route_single(&self, session_id: &SessionId, trigger: &NotifyTrigger) {
        let (trigger_type, summary, urgent) = match trigger {
            NotifyTrigger::ProcessExited {
                exit_code,
                command,
                duration_secs,
            } => {
                info!(
                    exit_code = exit_code,
                    command = ?command,
                    duration_secs = duration_secs,
                    channels = ?self.config.channels,
                    "routing ProcessExited notification"
                );
                let cmd_str = command.as_deref().unwrap_or("(unknown)");
                let summary = format!(
                    "Process `{cmd_str}` exited with code {exit_code} after {duration_secs:.1}s"
                );
                let urgent = *exit_code != 0;
                ("process_exited".to_string(), summary, urgent)
            }

            NotifyTrigger::WaitingForInput {
                prompt_type,
                prompt_text,
            } => {
                info!(
                    prompt_type = ?prompt_type,
                    prompt_text = ?prompt_text,
                    "routing WaitingForInput notification"
                );
                let prompt = prompt_text
                    .as_deref()
                    .unwrap_or("Terminal is waiting for input");
                let summary = format!("Waiting for input ({prompt_type:?}): {prompt}");
                let urgent = matches!(prompt_type, super::PromptType::Password);
                ("waiting_input".to_string(), summary, urgent)
            }

            NotifyTrigger::LongRunningDone {
                command,
                duration_secs,
                success,
            } => {
                info!(
                    command = ?command,
                    duration_secs = duration_secs,
                    success = success,
                    "routing LongRunningDone notification"
                );
                let cmd_str = command.as_deref().unwrap_or("Command");
                let status = if *success { "succeeded" } else { "failed" };
                let summary = format!("{cmd_str} {status} after {duration_secs:.1}s");
                ("long_running_done".to_string(), summary, !success)
            }

            NotifyTrigger::ErrorDetected { error_text } => {
                info!(
                    error_text = ?error_text,
                    "routing ErrorDetected notification"
                );
                let text = error_text.as_deref().unwrap_or("Unknown error");
                let summary = format!("Error detected: {text}");
                ("error_detected".to_string(), summary, true)
            }

            NotifyTrigger::AgentCompleted { session_id: sid } => {
                info!(
                    session_id = %sid,
                    "routing AgentCompleted notification"
                );
                self.event_bus
                    .publish_control(ControlEvent::AgentStatusChanged {
                        session_id: sid.clone(),
                        status: AgentStatus::Idle,
                    });
                ("agent_completed".to_string(), "Agent task completed".to_string(), false)
            }

            NotifyTrigger::AgentNeedsApproval { session_id: sid, tool } => {
                info!(
                    session_id = %sid,
                    tool = %tool,
                    "routing AgentNeedsApproval notification"
                );
                self.event_bus
                    .publish_control(ControlEvent::AgentStatusChanged {
                        session_id: sid.clone(),
                        status: AgentStatus::WaitingApproval,
                    });
                ("agent_needs_approval".to_string(), format!("Agent needs approval for tool: {tool}"), true)
            }

            NotifyTrigger::AgentError { session_id: sid, error } => {
                info!(
                    session_id = %sid,
                    error = %error,
                    "routing AgentError notification"
                );
                self.event_bus
                    .publish_control(ControlEvent::AgentError {
                        session_id: sid.clone(),
                        error: error.clone(),
                        class: ErrorClass::Transient,
                    });
                ("agent_error".to_string(), format!("Agent error: {error}"), true)
            }
        };

        // Emit unified NotificationTriggered control event
        self.event_bus
            .publish_control(ControlEvent::NotificationTriggered {
                session_id: session_id.clone(),
                trigger_type,
                summary,
                urgent,
            });

        debug!(
            channels = ?self.config.channels,
            sound = self.config.sound_enabled,
            "notification dispatched to channels"
        );
    }

    /// Check if a specific channel is enabled.
    pub fn has_channel(&self, channel: &str) -> bool {
        self.config.channels.iter().any(|c| c == channel)
    }

    /// Update the router configuration.
    pub fn update_config(&mut self, config: RouterConfig) {
        self.config = config;
    }
}
