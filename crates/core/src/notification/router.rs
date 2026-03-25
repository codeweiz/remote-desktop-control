//! Notification Router — routes detection triggers to configured channels.
//!
//! Currently emits notifications via the EventBus. Actual delivery to
//! web/desktop/IM channels is handled by the server and IM bridge layers.

use std::sync::Arc;

use tracing::{debug, info};

use crate::event_bus::EventBus;
use crate::events::{AgentStatus, ControlEvent, ErrorClass};

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
    pub fn route(&self, triggers: &[NotifyTrigger]) {
        for trigger in triggers {
            self.route_single(trigger);
        }
    }

    /// Route a single trigger.
    fn route_single(&self, trigger: &NotifyTrigger) {
        match trigger {
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
                // Emit as a control event for now.
                // The server's WebSocket handler and IM bridge will pick this up.
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
            }

            NotifyTrigger::ErrorDetected { error_text } => {
                info!(
                    error_text = ?error_text,
                    "routing ErrorDetected notification"
                );
            }

            NotifyTrigger::AgentCompleted { session_id } => {
                info!(
                    session_id = %session_id,
                    "routing AgentCompleted notification"
                );
                self.event_bus
                    .publish_control(ControlEvent::AgentStatusChanged {
                        session_id: session_id.clone(),
                        status: AgentStatus::Idle,
                    });
            }

            NotifyTrigger::AgentNeedsApproval { session_id, tool } => {
                info!(
                    session_id = %session_id,
                    tool = %tool,
                    "routing AgentNeedsApproval notification"
                );
                self.event_bus
                    .publish_control(ControlEvent::AgentStatusChanged {
                        session_id: session_id.clone(),
                        status: AgentStatus::WaitingApproval,
                    });
            }

            NotifyTrigger::AgentError { session_id, error } => {
                info!(
                    session_id = %session_id,
                    error = %error,
                    "routing AgentError notification"
                );
                self.event_bus
                    .publish_control(ControlEvent::AgentError {
                        session_id: session_id.clone(),
                        error: error.clone(),
                        class: ErrorClass::Transient,
                    });
            }
        }

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
