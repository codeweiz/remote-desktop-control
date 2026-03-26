// crates/core/src/agent/event.rs

use serde::{Deserialize, Serialize};

/// Supported agent providers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentKind {
    Claude,
    Gemini,
    OpenCode,
    Codex,
}

impl AgentKind {
    /// Resolve the CLI binary name for this agent kind.
    pub fn binary(&self) -> &str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Gemini => "gemini",
            AgentKind::OpenCode => "opencode",
            AgentKind::Codex => "npx",
        }
    }

    /// Command arguments to start this agent in ACP mode.
    pub fn acp_args(&self) -> Vec<&str> {
        match self {
            AgentKind::Claude => vec![
                "--input-format", "stream-json",
                "--output-format", "stream-json",
                "--verbose",
                "--dangerously-skip-permissions",
            ],
            AgentKind::Gemini => vec!["--experimental-acp"],
            AgentKind::OpenCode => vec!["acp"],
            AgentKind::Codex => vec!["@zed-industries/codex-acp"],
        }
    }

    /// Whether this agent uses native ACP protocol over stdio.
    pub fn is_native_acp(&self) -> bool {
        !matches!(self, AgentKind::Claude)
    }
}

impl std::fmt::Display for AgentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentKind::Claude => write!(f, "claude"),
            AgentKind::Gemini => write!(f, "gemini"),
            AgentKind::OpenCode => write!(f, "opencode"),
            AgentKind::Codex => write!(f, "codex"),
        }
    }
}

/// Events emitted by an agent backend, broadcast to all subscribers.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Text chunk from the agent (streaming).
    Text(String),
    /// Agent thinking/reasoning output.
    Thinking(String),
    /// Progress indicator (e.g., "Using tool: Bash").
    Progress(String),
    /// Tool invocation started.
    ToolUse {
        name: String,
        id: String,
        input: Option<String>,
    },
    /// Tool execution result.
    ToolResult {
        id: String,
        output: Option<String>,
        is_error: bool,
    },
    /// One prompt-response turn completed.
    TurnComplete {
        session_id: Option<String>,
        cost_usd: Option<f64>,
    },
    /// Error from the agent.
    Error(String),
}
