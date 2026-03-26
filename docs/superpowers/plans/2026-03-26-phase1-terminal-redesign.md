# Phase 1: Terminal System Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace raw shell PTY with tmux-backed sessions, switch WebSocket to binary frames, add OSC color interception, and improve mobile terminal input.

**Architecture:** tmux manages terminal sessions (scrollback, persistence, reattach). WebSocket uses Binary frames for PTY I/O (no base64). OSC 10/11 queries intercepted server-side for zero-latency TUI startup. Mobile gets a virtual keyboard bar.

**Tech Stack:** Rust (portable-pty + tmux), Axum WebSocket, React 19, xterm.js 5.5, @xterm/addon-canvas

**Spec:** `docs/superpowers/specs/2026-03-26-terminal-agent-redesign.md` — Phase 1

---

## File Map

### Files to Create
| File | Responsibility |
|------|---------------|
| `crates/core/src/pty/tmux.rs` | tmux command wrappers (validate, new-session, attach, kill, list, capture-pane) |
| `crates/core/src/pty/osc.rs` | OSC 10/11 color query interceptor with state machine |
| `web/src/components/MobileInputBar.tsx` | Virtual keyboard bar with Ctrl/Esc/Tab/arrow buttons |
| `web/src/hooks/useVisualViewportHeight.ts` | Detect virtual keyboard open/close on mobile |

### Files to Modify
| File | Changes |
|------|---------|
| `crates/core/src/pty/session.rs` | Rewrite: tmux-based spawn, simplified reader (no coalesce), live broadcast + status watch |
| `crates/core/src/pty/manager.rs` | tmux lifecycle: orphan cleanup, kill-session, update detector to use broadcast subscriber |
| `crates/core/src/pty/mod.rs` | Remove `buffer` module, add `tmux` and `osc` modules |
| `crates/core/src/events.rs` | Remove `PtyResized` variant (tmux handles resize internally) |
| `crates/core/src/lib.rs` | Add tmux validation in `CoreState::new()` |
| `crates/core/src/config.rs` | Remove `output_coalesce_ms`, `buffer_size`, and their env var overrides |
| `crates/core/Cargo.toml` | Keep `portable-pty` (still needed for PTY pair) |
| `crates/server/src/ws/terminal.rs` | Binary output frames, backpressure, keepalive, capture-pane on connect |
| `crates/server/src/api/sessions.rs` | Remove `get_session_buffer` handler (tmux handles scrollback) |
| `crates/server/src/router.rs` | Remove buffer endpoint route |
| `crates/server/src/lib.rs` | Call tmux orphan cleanup on startup |
| `crates/server/Cargo.toml` | Remove `base64` dependency |
| `crates/core/tests/pty_test.rs` | Update tests: remove RingBuffer tests, update create_session signature |
| `crates/plugin-host/src/im/mod.rs` | Update to use session broadcast subscriber instead of EventBus data subscriber |
| `web/src/hooks/useTerminal.ts` | Binary I/O, new xterm config (scrollback:0, cursorStyle:'bar'), Canvas fallback |
| `web/src/lib/types.ts` | Update WsMessage types, remove base64 types |
| `web/src/components/MobileView.tsx` | Integrate MobileInputBar |
| `web/package.json` | Add `@xterm/addon-canvas` |

### Files to Delete
| File | Reason |
|------|--------|
| `crates/core/src/pty/buffer.rs` | RingBuffer replaced by tmux scrollback |

---

## Task 1: tmux Command Wrappers

**Files:**
- Create: `crates/core/src/pty/tmux.rs`
- Modify: `crates/core/src/pty/mod.rs`

- [ ] **Step 1: Create `tmux.rs` with validation and session commands**

```rust
// crates/core/src/pty/tmux.rs
use std::process::Command;
use tracing::{debug, warn};

const TMUX_SESSION_PREFIX: &str = "rtb-";
const MIN_TMUX_VERSION: (u32, u32) = (2, 6);

/// Check tmux is installed and meets minimum version.
pub fn validate_tmux() -> anyhow::Result<()> {
    let output = Command::new("tmux")
        .arg("-V")
        .output()
        .map_err(|_| anyhow::anyhow!(
            "tmux not found in PATH. Install tmux >= {}.{}: \
             macOS: `brew install tmux`, \
             Ubuntu: `apt install tmux`, \
             Alpine: `apk add tmux`",
            MIN_TMUX_VERSION.0, MIN_TMUX_VERSION.1
        ))?;

    let version_str = String::from_utf8_lossy(&output.stdout);
    // Parse "tmux 3.4" or "tmux 2.6a"
    let version = version_str.trim().strip_prefix("tmux ").unwrap_or("");
    let parts: Vec<&str> = version.split('.').collect();
    let major: u32 = parts.first()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let minor: u32 = parts.get(1)
        .and_then(|s| s.trim_end_matches(|c: char| c.is_alphabetic()).parse().ok())
        .unwrap_or(0);

    if (major, minor) < MIN_TMUX_VERSION {
        anyhow::bail!(
            "tmux version {}.{} found, but >= {}.{} required",
            major, minor, MIN_TMUX_VERSION.0, MIN_TMUX_VERSION.1
        );
    }

    debug!(version = %version_str.trim(), "tmux validated");
    Ok(())
}

/// Format a tmux session name from session ID.
pub fn session_name(session_id: &str) -> String {
    format!("{}{}", TMUX_SESSION_PREFIX, session_id)
}

/// Create a new tmux session.
pub fn new_session(session_id: &str, cwd: &std::path::Path) -> anyhow::Result<()> {
    let name = session_name(session_id);
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", &name, "-c", &cwd.to_string_lossy()])
        .status()?;

    if !status.success() {
        anyhow::bail!("tmux new-session failed with exit code {:?}", status.code());
    }

    // Set environment variables inside the tmux session
    // (cannot use -e flag as it requires tmux 3.2+)
    let _ = Command::new("tmux")
        .args(["set-environment", "-t", &name, "TERM", "xterm-256color"])
        .status();
    let _ = Command::new("tmux")
        .args(["set-environment", "-t", &name, "COLORTERM", "truecolor"])
        .status();

    debug!(session_id, name = %name, "tmux session created");
    Ok(())
}

/// Kill a tmux session.
pub fn kill_session(session_id: &str) -> anyhow::Result<()> {
    let name = session_name(session_id);
    let status = Command::new("tmux")
        .args(["kill-session", "-t", &name])
        .status()?;

    if !status.success() {
        warn!(session_id, "tmux kill-session failed (session may already be dead)");
    }
    debug!(session_id, "tmux session killed");
    Ok(())
}

/// Check if a tmux session exists.
pub fn has_session(session_id: &str) -> bool {
    let name = session_name(session_id);
    Command::new("tmux")
        .args(["has-session", "-t", &name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// List all rtb-* tmux sessions.
pub fn list_rtb_sessions() -> Vec<String> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| l.starts_with(TMUX_SESSION_PREFIX))
                .map(|l| l.strip_prefix(TMUX_SESSION_PREFIX).unwrap_or(l).to_string())
                .collect()
        }
        _ => Vec::new(), // tmux server not running is fine
    }
}

/// Kill all rtb-* tmux sessions (startup orphan cleanup).
pub fn cleanup_orphans() {
    let sessions = list_rtb_sessions();
    if sessions.is_empty() {
        return;
    }
    warn!(count = sessions.len(), "cleaning up orphaned tmux sessions");
    for session_id in &sessions {
        let _ = kill_session(session_id);
    }
}

/// Resize a tmux session's active pane.
/// Uses resize-pane (available since tmux 1.x) instead of resize-window (tmux 2.9+).
pub fn resize_pane(session_id: &str, cols: u16, rows: u16) -> anyhow::Result<()> {
    let name = session_name(session_id);
    let _ = Command::new("tmux")
        .args(["resize-pane", "-t", &name, "-x", &cols.to_string(), "-y", &rows.to_string()])
        .status()?;
    Ok(())
}

/// Capture the visible content of the active pane.
/// Used on reconnect to send initial screen without full tmux redraw flood.
pub fn capture_pane(session_id: &str) -> anyhow::Result<Vec<u8>> {
    let name = session_name(session_id);
    let output = Command::new("tmux")
        .args(["capture-pane", "-t", &name, "-p"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("tmux capture-pane failed");
    }
    Ok(output.stdout)
}
```

- [ ] **Step 2: Update `pty/mod.rs` to declare new modules**

```rust
// crates/core/src/pty/mod.rs
pub mod manager;
pub mod osc;
pub mod session;
pub mod tmux;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p rtb-core`
Expected: Compiles (osc.rs can be empty stub for now)

- [ ] **Step 4: Create empty `osc.rs` stub**

```rust
// crates/core/src/pty/osc.rs
// OSC color responder — implemented in Task 4
```

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/pty/tmux.rs crates/core/src/pty/osc.rs crates/core/src/pty/mod.rs
git commit -m "feat(pty): add tmux command wrappers and module restructure"
```

---

## Task 2: Rewrite PtySession with tmux Backend

**Files:**
- Modify: `crates/core/src/pty/session.rs`
- Delete: `crates/core/src/pty/buffer.rs`
- Modify: `crates/core/src/events.rs`

- [ ] **Step 1: Delete `buffer.rs`**

```bash
rm crates/core/src/pty/buffer.rs
```

- [ ] **Step 2: Remove `PtyResized` from events.rs**

In `crates/core/src/events.rs`, remove the `PtyResized` variant from `DataEvent`:

```rust
pub enum DataEvent {
    PtyOutput { seq: u64, data: Bytes },
    PtyExited { exit_code: i32 },
    // REMOVED: PtyResized { cols: u16, rows: u16 },
    AgentMessage { seq: u64, content: AgentContent },
    AgentToolUse { seq: u64, tool: String, input: serde_json::Value },
}
```

- [ ] **Step 3: Rewrite `session.rs` — tmux-based spawn with live broadcast**

Replace entire `crates/core/src/pty/session.rs` with:

```rust
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use portable_pty::{native_pty_system, MasterPty, PtySize};
use tokio::sync::broadcast;
use tracing::{debug, error, warn};

use super::osc::OscColorResponder;
use super::tmux;

/// Capacity of the live broadcast channel per session.
const LIVE_BROADCAST_CAP: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum PtyStatus {
    Running,
    Exited(i32),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PtySessionInfo {
    pub id: String,
    pub name: String,
    pub status: PtyStatus,
    pub created_at: DateTime<Utc>,
    pub shell: String,
    pub cwd: PathBuf,
}

/// A tmux-backed PTY session.
///
/// On creation, the session:
/// 1. Creates a tmux session in the target directory
/// 2. Opens a PTY pair attached to `tmux attach`
/// 3. Starts a reader thread that broadcasts output via a channel
///
/// Reconnection: just open a new PTY pair with `tmux attach -d -t`.
/// tmux redraws the screen automatically.
pub struct PtySession {
    id: String,
    name: String,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    live_tx: broadcast::Sender<Bytes>,
    status: Arc<RwLock<PtyStatus>>,
    /// Watch channel for exit status — subscribers get notified when status changes.
    status_tx: tokio::sync::watch::Sender<PtyStatus>,
    status_rx: tokio::sync::watch::Receiver<PtyStatus>,
    created_at: DateTime<Utc>,
    cwd: PathBuf,
}

impl PtySession {
    /// Spawn a new tmux-backed PTY session.
    pub fn spawn(
        id: String,
        name: String,
        cwd: Option<&std::path::Path>,
    ) -> anyhow::Result<Arc<Self>> {
        let working_dir = cwd
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

        // 1. Create the tmux session
        tmux::new_session(&id, &working_dir)?;

        // 2. Open a PTY pair and attach to the tmux session
        let pty_system = native_pty_system();
        let size = PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 };
        let pair = pty_system.openpty(size)?;

        let tmux_name = tmux::session_name(&id);
        let mut cmd = portable_pty::CommandBuilder::new("tmux");
        cmd.args(["attach", "-d", "-t", &tmux_name]);

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave); // CRITICAL: close slave in parent

        let writer = pair.master.take_writer()?;
        let writer = Arc::new(Mutex::new(writer));
        let reader = pair.master.try_clone_reader()?;

        let (live_tx, _) = broadcast::channel(LIVE_BROADCAST_CAP);
        let status = Arc::new(RwLock::new(PtyStatus::Running));
        let (status_tx, status_rx) = tokio::sync::watch::channel(PtyStatus::Running);

        let session = Arc::new(Self {
            id: id.clone(),
            name,
            writer: writer.clone(),
            master: Mutex::new(pair.master),
            live_tx: live_tx.clone(),
            status: status.clone(),
            status_tx,
            status_rx,
            created_at: Utc::now(),
            cwd: working_dir,
        });

        // 3. Start reader thread (with writer ref for OSC responder)
        let reader_writer = writer;
        let reader_tx = live_tx;
        let reader_id = id.clone();
        std::thread::Builder::new()
            .name(format!("pty-reader-{}", id))
            .spawn(move || {
                Self::reader_loop(reader_id, reader, reader_writer, reader_tx);
            })?;

        // 4. Start child wait thread — updates both RwLock and watch channel
        let waiter_id = id;
        let waiter_status = status;
        let waiter_watch = session.status_tx.clone();
        std::thread::Builder::new()
            .name(format!("pty-waiter-{}", &waiter_id))
            .spawn(move || {
                let mut child = child;
                let exit_status = child.wait();
                let exit_code = match exit_status {
                    Ok(s) => s.exit_code() as i32,
                    Err(e) => {
                        error!(session_id = %waiter_id, error = %e, "error waiting for child");
                        -1
                    }
                };
                debug!(session_id = %waiter_id, exit_code, "PTY child exited");
                let new_status = PtyStatus::Exited(exit_code);
                if let Ok(mut s) = waiter_status.write() {
                    *s = new_status.clone();
                }
                let _ = waiter_watch.send(new_status);
            })?;

        Ok(session)
    }

    /// Background reader loop: reads PTY output, intercepts OSC, broadcasts.
    fn reader_loop(
        session_id: String,
        mut reader: Box<dyn Read + Send>,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        live_tx: broadcast::Sender<Bytes>,
    ) {
        let mut osc = OscColorResponder::new_dark_theme();
        osc.set_writer(writer);
        let mut buf = [0u8; 4096];

        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    debug!(session_id = %session_id, "PTY reader EOF");
                    break;
                }
                Ok(n) => {
                    let chunk = &buf[..n];
                    osc.intercept(chunk);
                    let _ = live_tx.send(Bytes::copy_from_slice(chunk));
                }
                Err(e) => {
                    if e.raw_os_error() == Some(libc::EIO) {
                        debug!(session_id = %session_id, "PTY reader EIO");
                    } else {
                        warn!(session_id = %session_id, error = %e, "PTY reader error");
                    }
                    break;
                }
            }
        }
    }

    pub fn id(&self) -> &str { &self.id }
    pub fn name(&self) -> &str { &self.name }
    pub fn cwd(&self) -> &PathBuf { &self.cwd }
    pub fn created_at(&self) -> DateTime<Utc> { self.created_at }

    pub fn status(&self) -> PtyStatus {
        self.status.read().unwrap().clone()
    }

    pub fn is_running(&self) -> bool {
        matches!(*self.status.read().unwrap(), PtyStatus::Running)
    }

    /// Subscribe to live output broadcast.
    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.live_tx.subscribe()
    }

    /// Subscribe to status changes (for exit notification).
    pub fn subscribe_status(&self) -> tokio::sync::watch::Receiver<PtyStatus> {
        self.status_rx.clone()
    }

    /// Write data to the PTY stdin.
    pub fn write_input(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Resize the tmux session.
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        tmux::resize_pane(&self.id, cols, rows)?;
        // Also resize the PTY pair so xterm.js and tmux agree
        let master = self.master.lock().unwrap();
        master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }

    /// Kill the session (tmux + PTY).
    pub fn kill(&self) -> anyhow::Result<()> {
        let _ = tmux::kill_session(&self.id);
        Ok(())
    }

    pub fn info(&self) -> PtySessionInfo {
        PtySessionInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            status: self.status(),
            created_at: self.created_at,
            shell: "tmux".to_string(),
            cwd: self.cwd.clone(),
        }
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p rtb-core`
Expected: May have errors in manager.rs due to removed buffer — fix in next task.

- [ ] **Step 5: Commit**

```bash
git add -A crates/core/src/pty/
git add crates/core/src/events.rs
git commit -m "feat(pty): rewrite PtySession with tmux backend, remove RingBuffer"
```

---

## Task 3: Update PtyManager for tmux Lifecycle

**Files:**
- Modify: `crates/core/src/pty/manager.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/config.rs`
- Modify: `crates/server/src/lib.rs`

- [ ] **Step 1: Rewrite `manager.rs` — remove buffer references, add tmux lifecycle**

The manager needs to:
- Remove all `RingBuffer` / `buffer_capacity` references
- `create_session` calls `PtySession::spawn(id, name, cwd)` (no shell param — tmux handles it)
- Update `start_detector_task()` to subscribe via `session.subscribe()` (broadcast) instead of `event_bus.create_data_subscriber()`
- Add `pub fn cleanup_orphans(&self)` that calls `tmux::cleanup_orphans()`
- Shutdown calls `tmux::kill_session` for each active session

- [ ] **Step 2: Update `config.rs` — remove obsolete PTY config fields**

In `crates/core/src/config.rs`:
- Remove `output_coalesce_ms` and `buffer_size` from `SessionConfig`
- Remove the env var override blocks for `RTB_SESSION_BUFFER_SIZE` and `RTB_SESSION_OUTPUT_COALESCE_MS`

- [ ] **Step 2b: Update `sessions.rs` API — remove buffer endpoint**

In `crates/server/src/api/sessions.rs`:
- Remove `get_session_buffer` handler (calls `session.buffer()` which no longer exists)
- Update `create_session` handler: remove `shell` parameter, `PtySession::spawn` no longer takes shell

In `crates/server/src/router.rs`:
- Remove the route for the buffer endpoint

- [ ] **Step 2c: Update test file**

In `crates/core/tests/pty_test.rs`:
- Remove all `RingBuffer` tests
- Update `create_session` calls to match new signature (no shell param)
- Remove `session.buffer()` calls

- [ ] **Step 2d: Update plugin-host IM bridge**

In `crates/plugin-host/src/im/mod.rs`:
- If it uses `event_bus.create_data_subscriber()` for PTY output, update to use `session.subscribe()` or keep using EventBus if manager still publishes there

- [ ] **Step 3: Update `CoreState::new()` in `lib.rs` — add tmux validation**

Add at the top of `CoreState::new()`:
```rust
// Validate tmux is available
crate::pty::tmux::validate_tmux()?;
```

- [ ] **Step 4: Update `crates/server/src/lib.rs` — orphan cleanup on startup**

After `CoreState::new()`, before binding the listener:
```rust
core.pty_manager.cleanup_orphans();
```

- [ ] **Step 5: Fix all compilation errors**

Run: `cargo check -p rtb-core -p rtb-server`
Fix any remaining references to removed types (`RingBuffer`, `buffer`, `coalesce_ms`, etc.)

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/pty/manager.rs crates/core/src/lib.rs crates/core/src/config.rs crates/server/src/lib.rs
git commit -m "feat(pty): update PtyManager for tmux lifecycle with orphan cleanup"
```

---

## Task 4: OSC Color Responder

**Files:**
- Modify: `crates/core/src/pty/osc.rs`

- [ ] **Step 1: Implement OSC responder with state machine**

```rust
// crates/core/src/pty/osc.rs
use std::io::Write;
use std::sync::{Arc, Mutex};

/// OSC 10/11 color query interceptor.
///
/// TUI apps (Neovim, Helix, bubbletea) query terminal foreground/background
/// colors via OSC 10;? and OSC 11;? escape sequences. Without interception,
/// these queries travel over WebSocket round-trip, adding latency.
///
/// This interceptor detects queries in the PTY output stream and writes
/// responses directly back to the PTY stdin, bypassing the network.
pub struct OscColorResponder {
    /// Pre-built response for OSC 10 (foreground color query)
    osc10_response: Vec<u8>,
    /// Pre-built response for OSC 11 (background color query)
    osc11_response: Vec<u8>,
    /// Writer to PTY stdin for sending responses
    writer: Option<Arc<Mutex<Box<dyn Write + Send>>>>,
    /// Partial sequence buffer for cross-boundary detection
    partial: Vec<u8>,
}

// OSC query patterns we're looking for:
// ESC ] 1 0 ; ? ESC \     (OSC 10 query with ST terminator)
// ESC ] 1 0 ; ? BEL       (OSC 10 query with BEL terminator)
// ESC ] 1 1 ; ? ESC \     (OSC 11 query with ST terminator)
// ESC ] 1 1 ; ? BEL       (OSC 11 query with BEL terminator)
const OSC10_ST: &[u8] = b"\x1b]10;?\x1b\\";
const OSC10_BEL: &[u8] = b"\x1b]10;?\x07";
const OSC11_ST: &[u8] = b"\x1b]11;?\x1b\\";
const OSC11_BEL: &[u8] = b"\x1b]11;?\x07";

impl OscColorResponder {
    /// Create a responder for dark theme (light text on dark background).
    pub fn new_dark_theme() -> Self {
        Self {
            // rgb:c8c8/c8c8/d8d8 ≈ #c8c8d8 (light gray foreground)
            osc10_response: b"\x1b]10;rgb:c8c8/c8c8/d8d8\x1b\\".to_vec(),
            // rgb:0d0d/0d0d/0d0d ≈ #0d0d0d (near-black background)
            osc11_response: b"\x1b]11;rgb:0d0d/1111/1717\x1b\\".to_vec(),
            writer: None,
            partial: Vec::new(),
        }
    }

    /// Set the writer for sending responses back to the PTY.
    pub fn set_writer(&mut self, writer: Arc<Mutex<Box<dyn Write + Send>>>) {
        self.writer = Some(writer);
    }

    /// Scan a chunk of PTY output for OSC 10/11 queries and respond.
    /// Must be called from the reader thread with each chunk.
    pub fn intercept(&mut self, chunk: &[u8]) {
        if self.writer.is_none() {
            return;
        }

        // Append chunk to partial buffer for cross-boundary matching
        self.partial.extend_from_slice(chunk);

        // Check for OSC 10 query
        if Self::contains_pattern(&self.partial, OSC10_ST)
            || Self::contains_pattern(&self.partial, OSC10_BEL)
        {
            self.respond_osc10();
        }

        // Check for OSC 11 query
        if Self::contains_pattern(&self.partial, OSC11_ST)
            || Self::contains_pattern(&self.partial, OSC11_BEL)
        {
            self.respond_osc11();
        }

        // Keep only last 16 bytes for cross-boundary matching
        // (longest pattern is 8 bytes, so 16 is generous)
        if self.partial.len() > 16 {
            let drain_to = self.partial.len() - 16;
            self.partial.drain(..drain_to);
        }
    }

    fn contains_pattern(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    fn respond_osc10(&self) {
        if let Some(ref writer) = self.writer {
            if let Ok(mut w) = writer.lock() {
                let _ = w.write_all(&self.osc10_response);
                let _ = w.flush();
            }
        }
    }

    fn respond_osc11(&self) {
        if let Some(ref writer) = self.writer {
            if let Ok(mut w) = writer.lock() {
                let _ = w.write_all(&self.osc11_response);
                let _ = w.flush();
            }
        }
    }
}
```

- [ ] **Step 2: Verify OSC wiring in PtySession**

The writer Arc and OSC responder are already wired into the `reader_loop` in Task 2's `session.rs` rewrite. Verify that `session.rs` passes the `writer` Arc to `reader_loop` and that `osc.set_writer(writer)` is called.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p rtb-core`

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/pty/osc.rs crates/core/src/pty/session.rs
git commit -m "feat(pty): add OSC 10/11 color query interceptor for TUI apps"
```

---

## Task 5: WebSocket Protocol — Backend

**Files:**
- Modify: `crates/server/src/ws/terminal.rs`
- Modify: `crates/server/Cargo.toml`

- [ ] **Step 1: Remove `base64` from server Cargo.toml**

Remove `base64 = "0.22"` from `crates/server/Cargo.toml` `[dependencies]`.

- [ ] **Step 2: Rewrite `ws/terminal.rs` — Binary output, backpressure, keepalive**

Replace `crates/server/src/ws/terminal.rs` with new implementation:

Key changes:
- On connect: send `tmux::capture_pane()` output as initial Binary frame (anti-reconnect-storm)
- PTY output sent as `Message::Binary(data)` — no base64, no JSON wrapping
- `Message::Binary` from client → PTY input (write to session)
- `Message::Text` from client → JSON command (resize, keepalive)
- Backpressure: if `broadcast::RecvError::Lagged` → close connection (client reconnects, gets capture-pane)
- Keepalive: respond to `{"type":"keepalive",...}` with `{"type":"keepalive_ack",...}`
- Subscribe to session's `broadcast::Receiver<Bytes>` instead of EventBus data subscriber
- Subscribe to session's `watch::Receiver<PtyStatus>` for exit notification
- Remove all replay logic (replay_gap, replay_done, last_seq)

```rust
// BEFORE the loop: send capture-pane as initial content (anti-reconnect-storm)
if let Ok(initial) = tmux::capture_pane(&session_id) {
    if !initial.is_empty() {
        let _ = ws_tx.send(Message::Binary(initial.into())).await;
    }
}

// Subscribe to live output and status
let mut live_rx = session.subscribe();
let mut status_rx = session.subscribe_status();

// Core loop:
loop {
    tokio::select! {
        // Live PTY output → Binary WebSocket frame
        output = live_rx.recv() => {
            match output {
                Ok(data) => {
                    if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // Client too slow — close and let them reconnect
                    warn!(session_id, lagged = n, "client lagging, closing");
                    break;
                }
                Err(_) => break,
            }
        }

        // Exit notification via watch channel
        _ = status_rx.changed() => {
            if let PtyStatus::Exited(code) = &*status_rx.borrow() {
                let msg = serde_json::json!({"type": "exit", "code": code});
                let _ = ws_tx.send(Message::Text(msg.to_string().into())).await;
                break;
            }
        }

        // Client message
        msg = ws_rx.next() => {
            match msg {
                Some(Ok(Message::Binary(data))) => {
                    // Raw PTY input
                    let _ = session.write_input(&data);
                }
                Some(Ok(Message::Text(text))) => {
                    // JSON command
                    if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&text) {
                        match cmd {
                            ClientCommand::Resize { cols, rows } => {
                                let _ = session.resize(cols, rows);
                            }
                            ClientCommand::Keepalive { client_time } => {
                                let ack = serde_json::json!({
                                    "type": "keepalive_ack",
                                    "server_time": chrono::Utc::now().timestamp_millis(),
                                });
                                let _ = ws_tx.send(Message::Text(ack.to_string().into())).await;
                            }
                        }
                    }
                }
                Some(Ok(Message::Close(_))) | None => break,
                _ => {}
            }
        }

        // Heartbeat ping every 30s
        _ = ping_interval.tick() => {
            if ws_tx.send(Message::Ping(vec![].into())).await.is_err() {
                break;
            }
        }
    }
}
```

- [ ] **Step 3: Update `ClientCommand` enum**

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientCommand {
    #[serde(rename = "resize")]
    Resize { cols: u16, rows: u16 },
    #[serde(rename = "keepalive")]
    Keepalive { client_time: Option<i64> },
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p rtb-server`

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/ws/terminal.rs crates/server/Cargo.toml
git commit -m "feat(ws): binary frames for PTY output, backpressure handling, keepalive"
```

---

## Task 6: WebSocket Protocol — Frontend

**Files:**
- Modify: `web/src/hooks/useTerminal.ts`
- Modify: `web/src/lib/types.ts`
- Modify: `web/package.json`

- [ ] **Step 1: Add `@xterm/addon-canvas` to package.json**

Run: `cd web && npm install @xterm/addon-canvas`

- [ ] **Step 2: Update `types.ts` — remove base64 types, add keepalive**

In `web/src/lib/types.ts`:
- Remove `TerminalOutput` (base64 data field)
- Remove `TerminalInput` (base64 data field)
- Add keepalive types:
```typescript
export interface KeepaliveMessage {
  type: 'keepalive'
  client_time: number
}

export interface KeepaliveAck {
  type: 'keepalive_ack'
  server_time: number
}

export interface TerminalExit {
  type: 'exit'
  code: number
}
```

- [ ] **Step 3: Rewrite `useTerminal.ts` — Binary I/O, new xterm config, Canvas fallback**

Key changes to `web/src/hooks/useTerminal.ts`:

```typescript
import { CanvasAddon } from '@xterm/addon-canvas'

// WebSocket: set binaryType IMMEDIATELY after construction
const ws = new WebSocket(url)
ws.binaryType = 'arraybuffer'  // MUST be before any onmessage fires

// xterm config changes:
const terminal = new Terminal({
  fontSize,
  fontFamily: "'JetBrains Mono', ui-monospace, monospace",
  cursorBlink: true,
  cursorStyle: 'bar',        // was 'block'
  scrollback: 0,             // was 10000 — tmux handles scrollback
  allowProposedApi: true,
})

// Renderer: WebGL → Canvas → DOM
try {
  const webgl = new WebglAddon()
  webgl.onContextLoss(() => {
    webgl.dispose()
    try { terminal.loadAddon(new CanvasAddon()) } catch { /* DOM fallback */ }
  })
  terminal.loadAddon(webgl)
} catch {
  try { terminal.loadAddon(new CanvasAddon()) } catch { /* DOM fallback */ }
}

// WebSocket: set binaryType
ws.binaryType = 'arraybuffer'

// Input: Binary frame
terminal.onData((data: string) => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(new TextEncoder().encode(data))
  }
})

// Output: Binary or Text
ws.onmessage = (event) => {
  if (event.data instanceof ArrayBuffer) {
    // Binary frame → PTY output
    terminal.write(new Uint8Array(event.data))
  } else if (typeof event.data === 'string') {
    // Text frame → control message
    try {
      const msg = JSON.parse(event.data)
      if (msg.type === 'exit') {
        terminal.writeln(`\r\n[Process exited with code ${msg.code}]`)
      } else if (msg.type === 'keepalive_ack') {
        // Connection health — could update latency display
      }
    } catch { /* ignore unparseable */ }
  }
}

// Keepalive: send every 10s
const keepaliveInterval = setInterval(() => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'keepalive', client_time: Date.now() }))
  }
}, 10000)

// Cleanup: clear keepalive interval in return
```

- [ ] **Step 4: Verify frontend builds**

Run: `cd web && npm run build`

- [ ] **Step 5: Commit**

```bash
git add web/src/hooks/useTerminal.ts web/src/lib/types.ts web/package.json web/package-lock.json
git commit -m "feat(web): binary WebSocket I/O, Canvas fallback, keepalive"
```

---

## Task 7: Mobile Input Bar

**Files:**
- Create: `web/src/hooks/useVisualViewportHeight.ts`
- Create: `web/src/components/MobileInputBar.tsx`
- Modify: `web/src/components/MobileView.tsx`

- [ ] **Step 1: Create `useVisualViewportHeight.ts`**

```typescript
// web/src/hooks/useVisualViewportHeight.ts
import { useState, useEffect } from 'react'

/** Tracks the visual viewport height to detect virtual keyboard open/close. */
export function useVisualViewportHeight(): number {
  const [height, setHeight] = useState(window.visualViewport?.height ?? window.innerHeight)

  useEffect(() => {
    const vv = window.visualViewport
    if (!vv) return

    const onResize = () => setHeight(vv.height)
    vv.addEventListener('resize', onResize)
    return () => vv.removeEventListener('resize', onResize)
  }, [])

  return height
}
```

- [ ] **Step 2: Create `MobileInputBar.tsx`**

```typescript
// web/src/components/MobileInputBar.tsx
import React, { useRef, useState } from 'react'
import { Box, IconButton, TextField } from '@mui/material'
import KeyboardReturnIcon from '@mui/icons-material/KeyboardReturn'
import KeyboardTabIcon from '@mui/icons-material/KeyboardTab'

interface MobileInputBarProps {
  onSend: (data: string) => void
}

const SPECIAL_KEYS = [
  { label: 'Esc', data: '\x1b' },
  { label: 'Tab', data: '\t' },
  { label: 'Ctrl', data: null }, // modifier
  { label: '\u2191', data: '\x1b[A' }, // Up
  { label: '\u2193', data: '\x1b[B' }, // Down
  { label: '\u2190', data: '\x1b[D' }, // Left
  { label: '\u2192', data: '\x1b[C' }, // Right
]

export default function MobileInputBar({ onSend }: MobileInputBarProps) {
  const [ctrlActive, setCtrlActive] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const handleKey = (data: string | null) => {
    if (data === null) {
      setCtrlActive(!ctrlActive)
      return
    }
    if (ctrlActive) {
      // Ctrl+key: send control character
      if (data.length === 1) {
        const code = data.toUpperCase().charCodeAt(0) - 64
        if (code > 0 && code < 32) {
          onSend(String.fromCharCode(code))
        }
      }
      setCtrlActive(false)
    } else {
      onSend(data)
    }
  }

  const handleTextSubmit = () => {
    const value = inputRef.current?.value ?? ''
    if (value) {
      onSend(value)
      if (inputRef.current) inputRef.current.value = ''
    }
    onSend('\r') // Enter
  }

  return (
    <Box sx={{
      display: 'flex',
      alignItems: 'center',
      gap: 0.5,
      p: 0.5,
      bgcolor: 'background.paper',
      borderTop: '1px solid',
      borderColor: 'divider',
    }}>
      {SPECIAL_KEYS.map(({ label, data }) => (
        <IconButton
          key={label}
          size="small"
          onClick={() => handleKey(data)}
          sx={{
            fontSize: 11,
            minWidth: 32,
            height: 28,
            borderRadius: 1,
            bgcolor: (label === 'Ctrl' && ctrlActive) ? 'primary.main' : 'action.hover',
            color: 'text.primary',
          }}
        >
          {label}
        </IconButton>
      ))}
      <TextField
        inputRef={inputRef}
        size="small"
        variant="outlined"
        placeholder="type..."
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            handleTextSubmit()
          }
        }}
        sx={{ flex: 1, '& input': { py: 0.25, fontSize: 12 } }}
      />
      <IconButton size="small" onClick={handleTextSubmit}>
        <KeyboardReturnIcon sx={{ fontSize: 16 }} />
      </IconButton>
    </Box>
  )
}
```

- [ ] **Step 3: Integrate into MobileView**

In `web/src/components/MobileView.tsx`, import and render `MobileInputBar` below the terminal when on a terminal tab. Pass `onSend` that writes to the WebSocket.

- [ ] **Step 4: Verify frontend builds**

Run: `cd web && npm run build`

- [ ] **Step 5: Commit**

```bash
git add web/src/hooks/useVisualViewportHeight.ts web/src/components/MobileInputBar.tsx web/src/components/MobileView.tsx
git commit -m "feat(mobile): add virtual keyboard bar with special keys"
```

---

## Task 8: Cleanup and Final Fixes

**Files:**
- Multiple files with remaining compilation errors

- [ ] **Step 1: Fix any remaining compilation errors in core**

Run: `cargo check -p rtb-core 2>&1`
Fix all errors — likely references to `buffer()`, `current_seq()`, `coalesce`, etc. in:
- `crates/core/src/pty/manager.rs`
- `crates/core/src/notification/detector.rs` (may reference session buffer)
- `crates/core/src/lib.rs` (CoreState initialization)

- [ ] **Step 2: Fix remaining compilation errors in server**

Run: `cargo check -p rtb-server 2>&1`
Fix all errors — likely references to `base64`, old `DataEvent` variants, replay logic in:
- `crates/server/src/ws/terminal.rs`
- `crates/server/src/api/sessions.rs` (may reference buffer endpoints)

- [ ] **Step 3: Fix frontend type errors**

Run: `cd web && npx tsc --noEmit 2>&1`
Fix references to old types (`TerminalOutput`, `TerminalInput`, etc.)

- [ ] **Step 4: Full build verification**

Run: `cargo build --workspace && cd web && npm run build`
Expected: Both compile cleanly.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix: resolve all compilation errors from terminal redesign"
```

---

## Task 9: Manual Integration Test

- [ ] **Step 1: Start the server**

```bash
cargo run -p rtb-cli
```
Expected: Server starts, logs "tmux validated", cleans up any orphan sessions.

- [ ] **Step 2: Open web UI and create a terminal session**

Open browser → create new terminal. Verify:
- Terminal renders correctly
- Can type commands and see output
- No double-enter issue
- Cursor position is correct
- Scrollback works via tmux (Ctrl+B, [)

- [ ] **Step 3: Test reconnection**

Refresh the browser page. Verify:
- Terminal reconnects automatically
- tmux redraws the screen with previous content
- Can continue typing

- [ ] **Step 4: Test resize**

Resize the browser window. Verify:
- Terminal re-fits correctly
- tmux pane resizes

- [ ] **Step 5: Test session deletion**

Delete the session via UI. Verify:
- tmux session is killed (`tmux list-sessions` shows it gone)

- [ ] **Step 6: Test TUI app (if available)**

Run `vim` or `htop` in the terminal. Verify:
- TUI renders correctly
- No lag on startup (OSC queries handled server-side)

- [ ] **Step 7: Commit any fixes from testing**

```bash
git add -A
git commit -m "fix: integration test fixes for terminal redesign"
```

---

## Completion Checklist

- [ ] tmux validation on startup works
- [ ] Orphan tmux sessions cleaned up on restart
- [ ] Terminal creates tmux session (rtb-* prefix)
- [ ] Binary WebSocket output (no base64)
- [ ] Binary WebSocket input (no JSON ambiguity)
- [ ] Keepalive ping/ack works
- [ ] WebGL → Canvas → DOM fallback chain
- [ ] OSC 10/11 queries intercepted server-side
- [ ] Session reconnection via tmux attach
- [ ] Session deletion kills tmux session
- [ ] Server shutdown kills all rtb-* sessions
- [ ] Mobile input bar renders with special keys
- [ ] No compilation errors in workspace
- [ ] Frontend builds cleanly
