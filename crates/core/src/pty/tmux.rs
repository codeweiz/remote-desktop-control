//! Tmux command wrappers for managing RTB terminal sessions.
//!
//! Every PTY session is backed by a tmux session named `rtb-{session_id}`.
//! This module provides thin wrappers around `tmux` CLI commands so the
//! rest of the codebase never has to construct tmux arguments directly.

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use tracing::{debug, info, warn};

/// Minimum tmux version we support.
const MIN_TMUX_VERSION: (u32, u32) = (2, 6);

/// Prefix used for all RTB-managed tmux sessions.
const SESSION_PREFIX: &str = "rtb-";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run a `tmux` command and return its `Output`.
fn run_tmux(args: &[&str]) -> Result<std::process::Output> {
    debug!(args = ?args, "running tmux command");
    Command::new("tmux")
        .args(args)
        .output()
        .context("failed to execute tmux")
}

/// Run a `tmux` command, returning an error if it exits non-zero.
fn run_tmux_ok(args: &[&str]) -> Result<std::process::Output> {
    let output = run_tmux(args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "tmux {} failed (exit {}): {}",
            args.first().unwrap_or(&""),
            output.status,
            stderr.trim()
        );
    }
    Ok(output)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate that tmux is installed and meets the minimum version requirement.
pub fn validate_tmux() -> Result<()> {
    let output = Command::new("tmux")
        .arg("-V")
        .output()
        .context("tmux is not installed or not in PATH")?;

    if !output.status.success() {
        bail!("tmux -V exited with non-zero status");
    }

    let version_str = String::from_utf8_lossy(&output.stdout);
    let version_str = version_str.trim();
    debug!(version = %version_str, "detected tmux version");

    // Expected format: "tmux 3.4" or "tmux 2.6"
    let numeric_part = version_str
        .strip_prefix("tmux ")
        .ok_or_else(|| anyhow!("unexpected tmux -V output: {version_str}"))?;

    let (major, minor) = parse_version(numeric_part)?;

    if (major, minor) < MIN_TMUX_VERSION {
        bail!(
            "tmux {major}.{minor} is too old; minimum required is {}.{}",
            MIN_TMUX_VERSION.0,
            MIN_TMUX_VERSION.1,
        );
    }

    info!(
        major,
        minor,
        "tmux version validated (>= {}.{})",
        MIN_TMUX_VERSION.0,
        MIN_TMUX_VERSION.1,
    );
    Ok(())
}

/// Parse a version string like "3.4" or "2.6a" into (major, minor).
fn parse_version(s: &str) -> Result<(u32, u32)> {
    // Strip any trailing letter suffix (e.g. "3.3a" -> "3.3")
    let cleaned: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let mut parts = cleaned.split('.');
    let major: u32 = parts
        .next()
        .ok_or_else(|| anyhow!("missing major version in '{s}'"))?
        .parse()
        .context("invalid major version")?;
    let minor: u32 = parts
        .next()
        .unwrap_or("0")
        .parse()
        .context("invalid minor version")?;
    Ok((major, minor))
}

/// Format the tmux session name for a given session id.
pub fn session_name(session_id: &str) -> String {
    format!("{SESSION_PREFIX}{session_id}")
}

/// Create a new detached tmux session.
///
/// Sets `TERM=xterm-256color` and `COLORTERM=truecolor` inside the session
/// via `tmux set-environment` (compatible with tmux >= 2.6, unlike `-e`
/// which requires 3.2+).
pub fn new_session(session_id: &str, cwd: &Path) -> Result<()> {
    let name = session_name(session_id);
    let cwd_str = cwd.to_string_lossy();

    info!(session = %name, cwd = %cwd_str, "creating tmux session");

    run_tmux_ok(&[
        "new-session",
        "-d",
        "-s",
        &name,
        "-c",
        &cwd_str,
    ])?;

    // Set environment variables inside the new session.
    run_tmux_ok(&["set-environment", "-t", &name, "TERM", "xterm-256color"])?;
    run_tmux_ok(&["set-environment", "-t", &name, "COLORTERM", "truecolor"])?;

    debug!(session = %name, "tmux session created and environment configured");
    Ok(())
}

/// Kill a tmux session.
pub fn kill_session(session_id: &str) -> Result<()> {
    let name = session_name(session_id);
    info!(session = %name, "killing tmux session");
    run_tmux_ok(&["kill-session", "-t", &name])?;
    Ok(())
}

/// Check whether a tmux session exists.
///
/// Returns `true` if the session exists, `false` otherwise.
/// Only returns `Err` on I/O failures (not on tmux reporting the session
/// is absent).
pub fn has_session(session_id: &str) -> Result<bool> {
    let name = session_name(session_id);
    let output = run_tmux(&["has-session", "-t", &name])?;
    Ok(output.status.success())
}

/// List all RTB-managed tmux session IDs.
///
/// Returns the extracted `session_id` portion (without the `rtb-` prefix).
pub fn list_rtb_sessions() -> Result<Vec<String>> {
    // `tmux list-sessions -F '#{session_name}'` prints one session name per
    // line. We filter to those starting with our prefix and strip it.
    let output = run_tmux(&["list-sessions", "-F", "#{session_name}"]);

    // If tmux has no server running it exits non-zero — treat as empty list.
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            debug!(error = %e, "tmux list-sessions failed; assuming no sessions");
            return Ok(Vec::new());
        }
    };

    if !output.status.success() {
        debug!("tmux list-sessions exited non-zero; assuming no sessions");
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let ids: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(SESSION_PREFIX))
        .map(String::from)
        .collect();

    debug!(count = ids.len(), "listed RTB tmux sessions");
    Ok(ids)
}

/// Kill all RTB-managed tmux sessions.
pub fn cleanup_orphans() -> Result<()> {
    let ids = list_rtb_sessions()?;
    if ids.is_empty() {
        debug!("no orphan RTB tmux sessions to clean up");
        return Ok(());
    }

    info!(count = ids.len(), "cleaning up orphan RTB tmux sessions");
    for id in &ids {
        if let Err(e) = kill_session(id) {
            warn!(session_id = %id, error = %e, "failed to kill orphan session");
        }
    }
    Ok(())
}

/// Resize the active pane in a tmux session.
///
/// Uses `resize-pane` (available since tmux 1.x) rather than
/// `resize-window` (which requires tmux 2.9+).
pub fn resize_pane(session_id: &str, cols: u16, rows: u16) -> Result<()> {
    let name = session_name(session_id);
    debug!(session = %name, cols, rows, "resizing tmux pane");

    run_tmux_ok(&[
        "resize-pane",
        "-t",
        &name,
        "-x",
        &cols.to_string(),
        "-y",
        &rows.to_string(),
    ])?;
    Ok(())
}

/// Capture the visible contents of the tmux pane.
///
/// Returns the raw stdout bytes from `tmux capture-pane -p`.
pub fn capture_pane(session_id: &str) -> Result<Vec<u8>> {
    let name = session_name(session_id);
    debug!(session = %name, "capturing tmux pane");

    let output = run_tmux_ok(&["capture-pane", "-t", &name, "-p"])?;
    Ok(output.stdout)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_name() {
        assert_eq!(session_name("abc123"), "rtb-abc123");
        assert_eq!(session_name(""), "rtb-");
    }

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("3.4").unwrap(), (3, 4));
        assert_eq!(parse_version("2.6").unwrap(), (2, 6));
        assert_eq!(parse_version("3.3a").unwrap(), (3, 3));
        assert_eq!(parse_version("4").unwrap(), (4, 0));
    }

    #[test]
    fn test_parse_version_invalid() {
        assert!(parse_version("").is_err());
        assert!(parse_version("abc").is_err());
    }
}
