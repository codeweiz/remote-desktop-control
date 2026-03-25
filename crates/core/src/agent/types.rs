//! ACP (Agent Communication Protocol) types.
//!
//! Defines the JSON-RPC message types specific to the ACP protocol
//! for communicating with AI agent subprocesses.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 base types (shared with plugin-host but kept local to
// avoid a circular dependency from core -> plugin-host)
// ---------------------------------------------------------------------------

/// JSON-RPC 2.0 version constant.
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC request ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Number(n) => write!(f, "{n}"),
            RequestId::String(s) => write!(f, "{s}"),
        }
    }
}

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub id: RequestId,
}

impl JsonRpcRequest {
    pub fn new(id: RequestId, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id,
        }
    }
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: RequestId,
}

impl JsonRpcResponse {
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// A JSON-RPC 2.0 error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 notification (no id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

// ---------------------------------------------------------------------------
// ACP-specific method names
// ---------------------------------------------------------------------------

/// ACP method names for JSON-RPC communication with agent subprocesses.
pub mod acp_methods {
    /// Host -> Agent: Initialize the agent session.
    pub const INITIALIZE: &str = "initialize";
    /// Host -> Agent: Create a new message (user turn).
    pub const MESSAGES_CREATE: &str = "messages/create";
    /// Agent -> Host: Streaming content notification.
    pub const MESSAGES_STREAM: &str = "messages/stream";
    /// Host -> Agent: Approve a tool use request.
    pub const TOOL_APPROVE: &str = "tool/approve";
    /// Host -> Agent: Deny a tool use request.
    pub const TOOL_DENY: &str = "tool/deny";
    /// Host -> Agent: Graceful shutdown.
    pub const SHUTDOWN: &str = "shutdown";
    /// Agent -> Host: Status change notification.
    pub const STATUS_CHANGED: &str = "status/changed";
    /// Agent -> Host: Error notification.
    pub const ERROR: &str = "error";
}

// ---------------------------------------------------------------------------
// ACP-specific parameter/result types
// ---------------------------------------------------------------------------

/// Parameters for `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpInitializeParams {
    /// Agent provider name (e.g., "claude-code", "aider").
    pub provider: String,
    /// Model identifier (e.g., "claude-sonnet-4-20250514").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Working directory for the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Protocol version.
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
}

/// Result of `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpInitializeResult {
    /// Agent name as reported by the agent.
    pub name: String,
    /// Agent version.
    pub version: String,
    /// Capabilities.
    #[serde(default)]
    pub capabilities: AcpCapabilities,
}

/// Agent capabilities discovered during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AcpCapabilities {
    /// Whether the agent supports streaming responses.
    #[serde(default)]
    pub streaming: bool,
    /// Whether the agent supports tool use.
    #[serde(default)]
    pub tool_use: bool,
    /// Available tools.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Supported content types.
    #[serde(default)]
    pub content_types: Vec<String>,
}

/// Parameters for `messages/create`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpMessagesCreateParams {
    /// The user's text input.
    pub text: String,
    /// Optional conversation/session context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

/// Notification params for `messages/stream`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpStreamNotification {
    /// The type of streaming event.
    #[serde(rename = "type")]
    pub event_type: AcpStreamEventType,
    /// Content payload (depends on event_type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<AcpStreamContent>,
}

/// Types of streaming events from the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpStreamEventType {
    /// Text content being streamed.
    Text,
    /// Agent is thinking/reasoning.
    Thinking,
    /// Agent wants to use a tool.
    ToolUse,
    /// Tool execution result.
    ToolResult,
    /// Stream is complete.
    Done,
    /// Error occurred during streaming.
    Error,
}

/// Content payload for streaming events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AcpStreamContent {
    /// Text content (for Text and Thinking events).
    Text {
        text: String,
        #[serde(default)]
        streaming: bool,
    },
    /// Tool use request (for ToolUse events).
    ToolUse {
        id: String,
        tool: String,
        input: serde_json::Value,
    },
    /// Tool result (for ToolResult events).
    ToolResult {
        id: String,
        output: String,
        #[serde(default)]
        is_error: bool,
    },
    /// Error content.
    Error {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
}

/// Parameters for `tool/approve`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpToolApproveParams {
    /// The tool use ID to approve.
    pub tool_id: String,
}

/// Parameters for `tool/deny`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpToolDenyParams {
    /// The tool use ID to deny.
    pub tool_id: String,
    /// Optional reason for denial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Notification params for `status/changed`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpStatusChangedParams {
    /// New agent status.
    pub status: String,
    /// Optional message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn default_protocol_version() -> String {
    "1.0".to_string()
}
