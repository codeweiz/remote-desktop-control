use bytes::Bytes;
use serde::{Deserialize, Serialize};

pub type SessionId = String;
pub type PluginId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionType {
    Terminal,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Initializing,
    Ready,
    Working,
    WaitingApproval,
    Idle,
    Crashed { error: String, class: ErrorClass },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorClass {
    Transient,
    Permanent,
}

/// Control events distributed via broadcast channel to all subscribers.
/// Used for low-frequency system-wide notifications.
#[derive(Debug, Clone)]
pub enum ControlEvent {
    SessionCreated {
        session_id: SessionId,
        session_type: SessionType,
    },
    SessionDeleted {
        session_id: SessionId,
    },
    SessionSwitched {
        session_id: SessionId,
    },
    AgentStatusChanged {
        session_id: SessionId,
        status: AgentStatus,
    },
    AgentError {
        session_id: SessionId,
        error: String,
        class: ErrorClass,
    },
    TunnelReady {
        url: String,
    },
    TunnelDown {
        reason: String,
    },
    PluginLoaded {
        plugin_id: PluginId,
        name: String,
    },
    PluginError {
        plugin_id: PluginId,
        error: String,
    },
    NotificationTriggered {
        session_id: SessionId,
        trigger_type: String,
        summary: String,
        urgent: bool,
    },
}

/// Data events sent through per-session mpsc channels.
/// Used for high-volume session-specific data.
#[derive(Debug, Clone)]
pub enum DataEvent {
    // Terminal (unchanged)
    PtyOutput {
        seq: u64,
        data: Bytes,
    },
    PtyExited {
        exit_code: i32,
    },
    // Agent events (expanded for Phase 2)
    /// User message sent to the agent (from web UI, IM, or API).
    AgentUserMessage {
        seq: u64,
        text: String,
        source: String,
    },
    AgentText {
        seq: u64,
        content: String,
        streaming: bool,
    },
    AgentThinking {
        seq: u64,
        content: String,
    },
    AgentToolUse {
        seq: u64,
        id: String,
        name: String,
        input: serde_json::Value,
    },
    AgentToolResult {
        seq: u64,
        id: String,
        output: String,
        is_error: bool,
    },
    AgentProgress {
        seq: u64,
        message: String,
    },
    AgentTurnComplete {
        seq: u64,
        cost_usd: Option<f64>,
    },
    AgentError {
        seq: u64,
        message: String,
        severity: ErrorClass,
        guidance: String,
    },
}
