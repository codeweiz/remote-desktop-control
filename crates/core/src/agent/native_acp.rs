//! Native ACP subprocess spawner for agents that speak ACP protocol directly
//! over stdin/stdout (Gemini, OpenCode, Codex).
//!
//! The caller uses the returned duplex streams to create a `ClientSideConnection`.
//! Claude uses a different bridge (`claude_sdk`) and should NOT be routed here.
//!
//! **NOTE:** `spawn_native_acp` must be called from within a `LocalSet`
//! because the bridging tasks use `tokio::task::spawn_local`.

use std::path::{Path, PathBuf};

use super::event::AgentKind;

/// Write the system prompt to the agent-specific location.
///
/// Returns `Ok(Some(path))` when the caller must set an env var pointing to
/// the file (Gemini), or `Ok(None)` when the file is written in a
/// conventional location the agent discovers on its own.
fn write_system_prompt(
    kind: &AgentKind,
    cwd: &Path,
    prompt: &str,
) -> Result<Option<PathBuf>, String> {
    match kind {
        AgentKind::Gemini => {
            // Gemini reads system instructions from the path in GEMINI_SYSTEM_MD.
            let path = cwd.join(".gemini_system.md");
            std::fs::write(&path, prompt)
                .map_err(|e| format!("Failed to write Gemini system prompt: {e}"))?;
            Ok(Some(path))
        }
        AgentKind::OpenCode => {
            // OpenCode picks up AGENTS.md from the workspace root.
            let path = cwd.join("AGENTS.md");
            std::fs::write(&path, prompt)
                .map_err(|e| format!("Failed to write OpenCode AGENTS.md: {e}"))?;
            Ok(None)
        }
        AgentKind::Codex => {
            // Codex reads .codex/instructions.md from the workspace root.
            let dir = cwd.join(".codex");
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("Failed to create .codex directory: {e}"))?;
            let path = dir.join("instructions.md");
            std::fs::write(&path, prompt)
                .map_err(|e| format!("Failed to write Codex instructions.md: {e}"))?;
            Ok(None)
        }
        AgentKind::Claude => {
            Err("Claude should use claude_bridge, not native_acp".into())
        }
    }
}

/// Spawn a native ACP agent subprocess and return duplex streams bridged to
/// its stdin/stdout.
///
/// The returned tuple is `(client_read, client_write)`:
/// - `client_read`  — read ACP JSON-RPC messages coming *from* the agent
/// - `client_write` — write ACP JSON-RPC messages going *to* the agent
///
/// # Panics
///
/// Will panic if called outside a `tokio::task::LocalSet` context (the
/// bridging tasks use `spawn_local`).
pub fn spawn_native_acp(
    kind: &AgentKind,
    cwd: &Path,
    system_prompt: Option<&str>,
) -> Result<(tokio::io::DuplexStream, tokio::io::DuplexStream), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    if matches!(kind, AgentKind::Claude) {
        return Err("Claude should use claude_bridge, not native_acp".into());
    }

    // ---- 1. Write system prompt if provided --------------------------------
    let system_md_path = if let Some(prompt) = system_prompt {
        write_system_prompt(kind, cwd, prompt)?
    } else {
        None
    };

    // ---- 2. Spawn the subprocess -------------------------------------------
    let binary = kind.binary();
    let args = kind.acp_args();

    eprintln!(
        "[native-acp] spawning {} {:?} in {:?}",
        binary, args, cwd
    );

    let mut cmd = tokio::process::Command::new(binary);
    cmd.args(&args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);

    // Gemini needs an env var pointing to the system prompt file.
    if let Some(ref path) = system_md_path {
        cmd.env("GEMINI_SYSTEM_MD", path);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn {binary}: {e}. Is {binary} installed?"))?;

    eprintln!(
        "[native-acp] {} process spawned pid={:?}",
        kind,
        child.id()
    );

    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("No stdout from {kind} process"))?;
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("No stdin from {kind} process"))?;

    // ---- 3+4. Bridge child stdout → client_read via duplex -----------------
    let (client_read, mut bridge_write) = tokio::io::duplex(64 * 1024);
    tokio::task::spawn_local(async move {
        let mut stdout = child_stdout;
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if bridge_write.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        // Keep child handle alive until the stdout bridge is done so the
        // process is not killed prematurely (kill_on_drop).
        drop(child);
    });

    // ---- 5. Bridge client_write → child stdin via duplex -------------------
    let (mut bridge_read, client_write) = tokio::io::duplex(64 * 1024);
    tokio::task::spawn_local(async move {
        let mut stdin = child_stdin;
        let mut buf = [0u8; 8192];
        loop {
            match bridge_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if stdin.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                    let _ = stdin.flush().await;
                }
                Err(_) => break,
            }
        }
    });

    // ---- 6. Return duplex streams ------------------------------------------
    Ok((client_read, client_write))
}
