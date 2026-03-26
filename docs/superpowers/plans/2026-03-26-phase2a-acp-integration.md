# Phase 2a: ACP SDK Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hand-written JSON-RPC agent protocol with the official ACP SDK, supporting Claude (via adapter), Gemini, OpenCode, and Codex (native ACP).

**Architecture:** Each agent runs on a dedicated thread with its own tokio runtime + LocalSet (ACP futures are `!Send`). A unified `AcpBackend` abstracts the agent kind, communicating via `mpsc` commands and `broadcast` events. Claude uses an in-process bridge (`AgentSideConnection` ↔ ClaudeSdk), while native agents pipe stdin/stdout to `ClientSideConnection`.

**Tech Stack:** `agent-client-protocol = "0.10.3"`, `tokio-util` (compat), `async-trait`, `futures` (compat)

**Spec:** `docs/superpowers/specs/2026-03-26-terminal-agent-redesign.md` — Phase 2, sections 2.1-2.2, 2.5

**Reference:** VibeAround at `/Users/zhouwei/Projects/github/VibeAround/src/core/src/agent/` — study `mod.rs`, `claude_sdk.rs`, `claude_acp.rs`, `gemini_acp.rs` for patterns.

---

## File Map

### Files to Create
| File | Responsibility |
|------|---------------|
| `crates/core/src/agent/acp_backend.rs` | Unified AcpBackend: dedicated thread, AcpCmd channel, event broadcast, Client trait impl |
| `crates/core/src/agent/claude_sdk.rs` | Claude CLI stream-json protocol: spawn, NDJSON parse, control handling |
| `crates/core/src/agent/claude_bridge.rs` | Claude ACP bridge: AgentSideConnection, duplex pipes, SDK↔ACP translation |
| `crates/core/src/agent/native_acp.rs` | Native ACP subprocess spawning for Gemini/OpenCode/Codex |
| `crates/core/src/agent/error_classify.rs` | Error classification (permanent/transient) with user guidance |
| `crates/core/src/agent/event.rs` | AgentEvent enum + AgentKind enum |

### Files to Modify
| File | Changes |
|------|---------|
| `crates/core/src/agent/mod.rs` | Re-export new modules, remove old module declarations |
| `crates/core/src/agent/manager.rs` | Rewrite: use AcpBackend, manage lifecycle, restart policy |
| `crates/core/src/events.rs` | Expand DataEvent with new agent variants per spec |
| `crates/core/Cargo.toml` | Add ACP SDK + tokio-util + async-trait dependencies |
| `crates/server/src/ws/agent.rs` | Update to use new AgentEvent types (Phase 2b will do full rewrite) |

### Files to Delete
| File | Reason |
|------|--------|
| `crates/core/src/agent/acp_client.rs` | Replaced by ACP SDK |
| `crates/core/src/agent/types.rs` | Replaced by ACP SDK types + agent/event.rs |

---

## Task 1: Add Dependencies and Core Types

**Files:**
- Modify: `crates/core/Cargo.toml`
- Create: `crates/core/src/agent/event.rs`
- Modify: `crates/core/src/events.rs`

- [ ] **Step 1: Add dependencies to core Cargo.toml**

```toml
agent-client-protocol = "0.10.3"
tokio-util = { version = "0.7", features = ["compat"] }
async-trait = "0.1"
futures = "0.3"
```

- [ ] **Step 2: Create `agent/event.rs` with AgentEvent and AgentKind**

```rust
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
```

- [ ] **Step 3: Expand DataEvent in events.rs**

Add new variants per spec (keep existing PtyOutput/PtyExited/AgentMessage/AgentToolUse, add new ones):

```rust
pub enum DataEvent {
    // Terminal (unchanged)
    PtyOutput { seq: u64, data: Bytes },
    PtyExited { exit_code: i32 },
    // Agent (expanded)
    AgentText { seq: u64, content: String, streaming: bool },
    AgentThinking { seq: u64, content: String },
    AgentToolUse { seq: u64, id: String, name: String, input: serde_json::Value },
    AgentToolResult { seq: u64, id: String, output: String, is_error: bool },
    AgentProgress { seq: u64, message: String },
    AgentTurnComplete { seq: u64, cost_usd: Option<f64> },
    AgentError { seq: u64, message: String, severity: ErrorClass, guidance: String },
}
```

Remove the old `AgentMessage` and `AgentToolUse` variants (replaced by the expanded set above).

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p rtb-core`
Expected: Errors in agent/manager.rs and server/ws/agent.rs due to removed types. That's OK — later tasks fix them.

- [ ] **Step 5: Commit**

```
git commit -m "feat(agent): add ACP SDK dependency and new event types"
```

---

## Task 2: Error Classification

**Files:**
- Create: `crates/core/src/agent/error_classify.rs`

- [ ] **Step 1: Implement error classifier**

```rust
// crates/core/src/agent/error_classify.rs

use crate::events::ErrorClass;

/// Classify an agent error and provide user guidance.
pub fn classify_error(stderr: &str, error_msg: &str) -> (ErrorClass, String) {
    let combined = format!("{}\n{}", stderr, error_msg).to_lowercase();

    if combined.contains("module_not_found")
        || combined.contains("eacces")
        || combined.contains("permission denied")
    {
        (ErrorClass::Permanent, "Agent binary not available or permission denied. Check installation.".into())
    } else if combined.contains("enoent") || combined.contains("not found") {
        (ErrorClass::Permanent, "Agent command not found. Ensure it is installed and in PATH.".into())
    } else if combined.contains("syntax") || combined.contains("invalid") {
        (ErrorClass::Permanent, "Configuration error. Check agent settings.".into())
    } else if combined.contains("timeout") || combined.contains("econnrefused") || combined.contains("timed out") {
        (ErrorClass::Transient, "Network timeout. Will retry automatically.".into())
    } else if combined.contains("rate limit") || combined.contains("429") {
        (ErrorClass::Transient, "Rate limited. Will retry after backoff.".into())
    } else {
        (ErrorClass::Transient, "Unknown error. Will attempt restart.".into())
    }
}
```

- [ ] **Step 2: Commit**

```
git commit -m "feat(agent): add error classification with user guidance"
```

---

## Task 3: Claude SDK Protocol Parser

**Files:**
- Create: `crates/core/src/agent/claude_sdk.rs`

This is the bridge to Claude Code's proprietary `stream-json` protocol. Study `/Users/zhouwei/Projects/github/VibeAround/src/core/src/agent/claude_sdk.rs` carefully — port the key logic.

- [ ] **Step 1: Implement ClaudeSdk**

Key design (following VibeAround):
- `ClaudeSdk::spawn(cwd, system_prompt, resume_session_id)` → spawns `claude --input-format stream-json --output-format stream-json --verbose --dangerously-skip-permissions`
- Reads NDJSON from stdout in a background task
- Parses message types: `system`, `assistant`, `result`, `control_request`
- Auto-handles `can_use_tool` with `{"behavior": "allow"}` response
- Auto-handles `hook_callback` with success response
- Emits `SdkEvent` variants: `AssistantMessage`, `TurnResult`, `SystemInit`, `ControlHandled`
- `send_user_message(text)` writes JSON to stdin
- `recv_event()` returns next SdkEvent from channel

ContentBlock enum:
```rust
pub enum ContentBlock {
    Text { text: String },
    Thinking { text: String },
    ToolUse { id: String, name: String, input: Option<String> },
    ToolResult { id: String, output: Option<String>, is_error: bool },
}
```

SdkEvent enum:
```rust
pub enum SdkEvent {
    AssistantMessage { content: Vec<ContentBlock> },
    TurnResult { session_id: Option<String>, is_error: bool, error_text: Option<String> },
    SystemInit { session_id: Option<String> },
    ControlHandled { subtype: String },
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p rtb-core`

- [ ] **Step 3: Commit**

```
git commit -m "feat(agent): implement Claude CLI stream-json protocol parser"
```

---

## Task 4: Claude ACP Bridge

**Files:**
- Create: `crates/core/src/agent/claude_bridge.rs`

Study `/Users/zhouwei/Projects/github/VibeAround/src/core/src/agent/claude_acp.rs` — port the bridge pattern.

- [ ] **Step 1: Implement ClaudeAcpBridge**

Key design:
- `spawn_claude_acp(cwd, system_prompt)` → creates duplex pipes, spawns dedicated thread with LocalSet
- `ClaudeAcpBridge` struct implements ACP `Agent` trait (via `MessageHandler<AgentSide>`)
- `initialize()` → spawns ClaudeSdk lazily, returns protocol version
- `new_session()` → returns stable bridge session ID
- `prompt()` → extracts text from ContentBlocks, sends via ClaudeSdk, drains events until TurnResult
- Translates ContentBlock → ACP SessionNotification via notification channel
- Returns `(DuplexStream, DuplexStream, JoinHandle)` for the caller to create a ClientSideConnection

The bridge uses `AgentSideConnection` (not `ClientSideConnection`) because it *acts as an agent* to the ACP client.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p rtb-core`

- [ ] **Step 3: Commit**

```
git commit -m "feat(agent): implement Claude ACP adapter bridge"
```

---

## Task 5: Native ACP Subprocess Spawning

**Files:**
- Create: `crates/core/src/agent/native_acp.rs`

Study `/Users/zhouwei/Projects/github/VibeAround/src/core/src/agent/gemini_acp.rs` — port the pattern.

- [ ] **Step 1: Implement spawn functions for native ACP agents**

```rust
// crates/core/src/agent/native_acp.rs

/// Spawn a native ACP agent subprocess and return duplex streams.
/// Used for Gemini, OpenCode, Codex — they speak ACP natively over stdio.
pub fn spawn_native_acp(
    kind: &AgentKind,
    cwd: &Path,
    system_prompt: Option<&str>,
) -> Result<(DuplexStream, DuplexStream), String>
```

Key design (following VibeAround):
- Spawn `tokio::process::Command` with the agent binary + ACP args
- Pipe stdin/stdout
- Bridge child stdout → duplex read via spawn_local task
- Bridge duplex write → child stdin via spawn_local task
- For Gemini: set `GEMINI_SYSTEM_MD` env var pointing to a temp file with system prompt
- For OpenCode: write system prompt to `AGENTS.md` in cwd
- For Codex: write to `.codex/instructions.md` in cwd
- Return duplex stream pair for the caller to create ClientSideConnection

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p rtb-core`

- [ ] **Step 3: Commit**

```
git commit -m "feat(agent): add native ACP subprocess spawning (Gemini/OpenCode/Codex)"
```

---

## Task 6: Unified AcpBackend

**Files:**
- Create: `crates/core/src/agent/acp_backend.rs`

This is the main integration point. Study `/Users/zhouwei/Projects/github/VibeAround/src/core/src/agent/mod.rs` lines 148-430.

- [ ] **Step 1: Implement AcpBackend**

```rust
pub struct AcpBackend {
    kind: AgentKind,
    event_tx: broadcast::Sender<AgentEvent>,
    cmd_tx: Option<mpsc::Sender<AcpCmd>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

enum AcpCmd {
    Prompt { text: String, done_tx: oneshot::Sender<Result<(), String>> },
    Shutdown,
}
```

Key design:
- `AcpBackend::new(kind)` → creates broadcast channel, no thread yet
- `start(cwd, system_prompt)` → spawns dedicated thread with LocalSet:
  1. For Claude: calls `claude_bridge::spawn_claude_acp()` → gets duplex streams → creates `ClientSideConnection`
  2. For native: calls `native_acp::spawn_native_acp()` → gets duplex streams → creates `ClientSideConnection`
  3. Runs ACP `Initialize` → `NewSession` handshake
  4. Enters command loop: receives `AcpCmd::Prompt`, calls `conn.prompt()`, sends `TurnComplete` event
- `send_message(text)` → sends `AcpCmd::Prompt`, blocks on done_tx
- `send_message_fire(text)` → sends `AcpCmd::Prompt`, ignores done_tx
- `subscribe()` → returns `broadcast::Receiver<AgentEvent>`
- `shutdown()` → sends `AcpCmd::Shutdown`, joins thread

The `SharedAcpClientHandler` (implementing ACP `Client` trait):
- `request_permission()` → auto-approve first option
- `session_notification()` → translate ACP notifications to AgentEvent, send via event_tx

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p rtb-core`

- [ ] **Step 3: Commit**

```
git commit -m "feat(agent): implement unified AcpBackend with dedicated thread model"
```

---

## Task 7: Rewrite AgentManager

**Files:**
- Modify: `crates/core/src/agent/manager.rs`
- Delete: `crates/core/src/agent/acp_client.rs`
- Delete: `crates/core/src/agent/types.rs`
- Modify: `crates/core/src/agent/mod.rs`

- [ ] **Step 1: Delete old files**

```bash
rm crates/core/src/agent/acp_client.rs crates/core/src/agent/types.rs
```

- [ ] **Step 2: Update `agent/mod.rs`**

```rust
pub mod acp_backend;
pub mod claude_bridge;
pub mod claude_sdk;
pub mod error_classify;
pub mod event;
pub mod manager;
pub mod native_acp;
```

- [ ] **Step 3: Rewrite `agent/manager.rs`**

New AgentManager design:
```rust
pub struct AgentManager {
    agents: DashMap<SessionId, ManagedAgent>,
    event_bus: Arc<EventBus>,
}

struct ManagedAgent {
    backend: AcpBackend,
    name: String,
    kind: AgentKind,
    cwd: PathBuf,
    created_at: DateTime<Utc>,
    restart_count: u32,
    companion_terminal_id: Option<String>,
}
```

Key methods:
- `create_agent(session_id, name, kind, cwd, system_prompt)` → creates AcpBackend, starts it, starts event router
- `send_message(session_id, text)` → delegates to backend.send_message_fire()
- `kill_agent(session_id)` → shutdown backend, remove from map
- `list_agents()` → agent info list
- `has_agent()`, `agent_count()`
- `shutdown_all()` → shutdown all backends

Event router: subscribes to backend.subscribe(), translates AgentEvent to DataEvent, publishes to event_bus for the session.

Restart logic: on backend error, classify error, if transient → schedule restart with backoff (max 3 attempts, 3s→6s→12s→30s).

- [ ] **Step 4: Fix compilation errors in ws/agent.rs**

Make minimal fixes to `crates/server/src/ws/agent.rs` so it compiles. The full rewrite is in Phase 2b.

- [ ] **Step 5: Verify full workspace compilation**

Run: `cargo check --workspace`

- [ ] **Step 6: Commit**

```
git commit -m "feat(agent): rewrite AgentManager with ACP SDK backend"
```

---

## Task 8: Compilation Cleanup and Smoke Test

**Files:**
- Various — fix all remaining compilation issues

- [ ] **Step 1: Fix all compilation errors**

Run: `cargo check --workspace 2>&1`
Fix errors in:
- `crates/server/src/ws/agent.rs` (old DataEvent::AgentMessage references)
- `crates/server/src/api/sessions.rs` (agent creation may reference old API)
- `crates/core/tests/` (any agent test references)
- `crates/plugin-host/` (if it references old agent types)

- [ ] **Step 2: Run tests**

Run: `cargo test --workspace`

- [ ] **Step 3: Build frontend**

Run: `cd web && npm run build`

- [ ] **Step 4: Commit**

```
git commit -m "fix: resolve all compilation errors from ACP SDK migration"
```

---

## Task 9: Integration Smoke Test

- [ ] **Step 1: Verify agent binary availability**

```bash
which claude && claude --version
which gemini && gemini --version
```

- [ ] **Step 2: Start server and create an agent session via API**

```bash
TOKEN=$(cat ~/.rtb/session.token)
curl -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"name":"test-agent","type":"agent","provider":"claude"}' \
  http://127.0.0.1:3000/api/v1/sessions
```

Verify:
- Agent creates without error (or shows clear error if claude not installed)
- Error classification provides helpful message if binary not found

- [ ] **Step 3: Send a message to the agent**

Connect to WebSocket and send:
```json
{"type":"message","text":"What is 2+2?"}
```

Verify agent responds (if binary available).

- [ ] **Step 4: Commit any fixes**

```
git commit -m "fix: integration test fixes for ACP agent"
```

---

## Completion Checklist

- [ ] `agent-client-protocol` crate integrated and compiling
- [ ] Claude adapter: ClaudeSdk (stream-json) + ClaudeAcpBridge (Agent impl)
- [ ] Native ACP: Gemini, OpenCode, Codex subprocess spawning
- [ ] AcpBackend: dedicated thread + LocalSet, cmd channel, event broadcast
- [ ] AgentManager: create/kill/list agents, event routing, restart policy
- [ ] Error classification: permanent vs transient with user guidance
- [ ] Old hand-written acp_client.rs and types.rs deleted
- [ ] DataEvent expanded with new agent variants
- [ ] Workspace compiles, tests pass, frontend builds
