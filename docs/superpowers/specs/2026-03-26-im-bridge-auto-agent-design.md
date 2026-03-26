# IM Bridge Auto-Agent Assignment

**Date:** 2026-03-26
**Status:** Approved

## Problem

When a Feishu user sends a message, IM Bridge replies "Not attached to any session. Use /attach <session_id> first." This requires users to know internal session IDs and manually create agents via CLI. The UX should be: send a message, get an agent response.

## Design

### Behavior

**Plain text message (no `/` prefix):**
1. If channel has an attached agent → call `agent_manager.send_message(session_id, text)` to forward
2. If no agent attached → auto-create an agent, attach to channel, then forward the message

**Simplified commands:**

| Command | Action |
|---------|--------|
| `/new [provider]` | Create new agent (default: claude-code), auto-attach |
| `/list` | List all agents with numbered index: `#1 IM-Agent-abc [Ready]` |
| `/switch N` | Switch channel to agent #N from the list |
| `/help` | Show commands |
| plain text | Forward to attached agent (auto-create if none) |

**Removed commands:** `/attach`, `/detach`, `/agent create`, `/agent chat`, `/agent list`, `/sessions`, `/task add`, `/task list`

### Architecture

1. **`PluginManager` takes `Arc<CoreState>` instead of `Arc<EventBus>`**
   - `CoreState` contains `event_bus`, `agent_manager`, etc.
   - `ImBridge` receives `Arc<CoreState>` and uses `core.event_bus` where it previously used `event_bus`, plus `core.agent_manager` for agent operations

2. **`ImBridge` stores `Arc<CoreState>`** (replaces the `event_bus` field)

3. **Agent creation flow** (in `handle_command` for both auto-create and `/new`):
   ```
   session_id = nanoid!(10)
   name = "IM-Agent-{short_id}"
   provider = "claude-code" (or user-specified)
   cwd = std::env::current_dir()
   core.agent_manager.create_agent(session_id, name, provider, "", cwd).await
   channel_sessions.insert(channel, session_id)
   ```

4. **Message forwarding** (in PlainText handler):
   ```
   core.agent_manager.send_message(session_id, text).await
   ```

5. **Agent listing** (for `/list` and `/switch`):
   ```
   core.agent_manager.list_agents() -> Vec<(SessionId, name, status, created_at)>
   ```

### Files Changed

| File | Change |
|------|--------|
| `crates/plugin-host/src/manager.rs` | `PluginManager::new()` takes `Arc<CoreState>` instead of `Arc<EventBus>` |
| `crates/plugin-host/src/im/mod.rs` | Store `Arc<CoreState>`, rewrite commands, auto-create agent |
| `crates/cli/src/commands/start.rs` | Pass `Arc<CoreState>` to PluginManager |
| `crates/tauri-app/src/commands.rs` | Pass `Arc<CoreState>` to PluginManager |

### What Is NOT Changed

- Agent event broadcasting (already works via `start_notification_listener`)
- Outgoing message throttling
- ANSI stripping
- The plugin sender mechanism
