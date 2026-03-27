//! Detection Engine and Notification Routing.
//!
//! Implements a three-layer signal fusion detector for identifying notable events
//! (process completion, errors, prompts) and a router to dispatch notifications
//! to configured channels via the EventBus.

pub mod detector;
pub mod router;
pub mod store;

use serde::{Deserialize, Serialize};

/// Notification trigger types that can be detected and routed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifyTrigger {
    /// A process (shell command) exited.
    ProcessExited {
        exit_code: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        duration_secs: f64,
    },
    /// The terminal is waiting for user input (prompt detected).
    WaitingForInput {
        prompt_type: PromptType,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_text: Option<String>,
    },
    /// A long-running command completed.
    LongRunningDone {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        duration_secs: f64,
        success: bool,
    },
    /// An error was detected in the output.
    ErrorDetected {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_text: Option<String>,
    },
    /// An agent task completed.
    AgentCompleted { session_id: String },
    /// An agent needs human approval for a tool use.
    AgentNeedsApproval { session_id: String, tool: String },
    /// An agent encountered an error.
    AgentError { session_id: String, error: String },
}

/// Types of prompts detected by the semantic layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptType {
    /// Yes/No confirmation prompt.
    Confirmation,
    /// Password or secret input prompt.
    Password,
    /// General input prompt.
    Input,
    /// Selection menu.
    Selection,
}

/// Confidence score from a detection layer.
#[derive(Debug, Clone)]
pub struct DetectionSignal {
    /// Which layer produced this signal.
    pub layer: DetectionLayer,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// The trigger that was detected.
    pub trigger: NotifyTrigger,
    /// Weight for signal fusion.
    pub weight: f64,
}

/// Which detection layer produced a signal.
#[derive(Debug, Clone)]
pub enum DetectionLayer {
    /// Layer 1: Process monitor (foreground process tracking).
    ProcessMonitor,
    /// Layer 2: Timing analysis (output rate tracking).
    Timing,
    /// Layer 3: Semantic analysis (pattern matching).
    Semantic,
}

/// Threshold for signal fusion to trigger a notification.
pub const FUSION_THRESHOLD: f64 = 0.7;
