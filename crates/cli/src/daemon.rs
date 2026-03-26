use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rtb_core::config::Config;
use rtb_core::session::store::SessionStore;
use rtb_core::session::types::{SessionStatus, SessionType};

// ---------------------------------------------------------------------------
// Token management
// ---------------------------------------------------------------------------

/// Generate a 256-bit random token encoded as a 64-character hex string.
pub fn generate_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

/// Load an existing token from the configured path, or generate and persist
/// a fresh one. The token file is created with mode 0600 on Unix.
pub fn load_or_create_token(config: &Config) -> Result<String> {
    let token_path = expand_tilde(&config.security.token_file);

    // Try to read an existing token
    if let Ok(token) = std::fs::read_to_string(&token_path) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Generate a new token
    let token = generate_token();

    // Create parent dir if needed
    if let Some(parent) = Path::new(&token_path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    std::fs::write(&token_path, &token)
        .with_context(|| format!("failed to write token to {}", token_path))?;

    // Restrict token file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

/// Read the token from the config-defined path.  Returns an error if the
/// file does not exist or is empty (i.e. the daemon has never been started).
pub fn read_token(config: &Config) -> Result<String> {
    let token_path = expand_tilde(&config.security.token_file);
    let token = std::fs::read_to_string(&token_path)
        .with_context(|| format!("cannot read token file {} – is the daemon running?", token_path))?
        .trim()
        .to_string();
    if token.is_empty() {
        anyhow::bail!("token file {} is empty", token_path);
    }
    Ok(token)
}

// ---------------------------------------------------------------------------
// PID file management
// ---------------------------------------------------------------------------

/// Canonical path to the PID file.
fn pid_file_path() -> Result<PathBuf> {
    Ok(Config::rtb_dir()?.join("rtb.pid"))
}

/// Write the current process's PID to `~/.rtb/rtb.pid`.
pub fn write_pid_file() -> Result<()> {
    let path = pid_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, std::process::id().to_string())
        .with_context(|| format!("failed to write PID file {}", path.display()))?;
    Ok(())
}

/// Remove the PID file if it exists.
pub fn remove_pid_file() -> Result<()> {
    let path = pid_file_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Read the PID from the PID file.  Returns `None` if the file is absent or
/// cannot be parsed.
fn read_pid() -> Option<u32> {
    let path = pid_file_path().ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    content.trim().parse().ok()
}

/// Check whether the daemon is running by examining the PID file and
/// querying the OS for a live process with that PID.
pub fn is_running() -> bool {
    match read_pid() {
        Some(pid) => process_alive(pid),
        None => false,
    }
}

/// Send SIGTERM to the daemon process (read from the PID file) and remove
/// the PID file afterwards.
pub fn stop_daemon() -> Result<()> {
    let pid = read_pid().context("no PID file found – daemon does not appear to be running")?;

    if !process_alive(pid) {
        // Stale PID file – clean up and return.
        remove_pid_file()?;
        println!("Removed stale PID file (process {} not found).", pid);
        return Ok(());
    }

    #[cfg(unix)]
    {
        // Send SIGTERM
        unsafe {
            libc_kill(pid);
        }
        println!("Sent SIGTERM to RTB daemon (PID {}).", pid);
    }

    #[cfg(not(unix))]
    {
        anyhow::bail!(
            "stopping the daemon is only supported on Unix. \
             Please kill process {} manually.",
            pid
        );
    }

    remove_pid_file()?;
    Ok(())
}

/// Print the daemon status.
pub fn print_status() -> Result<()> {
    if let Some(pid) = read_pid() {
        if process_alive(pid) {
            println!("RTB daemon is running (PID {}).", pid);
        } else {
            println!("RTB daemon is NOT running (stale PID file for PID {}).", pid);
        }
    } else {
        println!("RTB daemon is NOT running.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Crash recovery: stale PID detection
// ---------------------------------------------------------------------------

/// Check for a stale PID file (PID file exists but process is dead).
/// If found, removes the stale PID file and logs a warning.
/// Returns `true` if a stale PID file was cleaned up.
pub fn check_stale_pid() -> Result<bool> {
    let path = pid_file_path()?;
    if !path.exists() {
        return Ok(false);
    }

    if let Some(pid) = read_pid() {
        if !process_alive(pid) {
            tracing::warn!(
                pid = pid,
                "Found stale PID file (process {} is not running). Cleaning up.",
                pid
            );
            remove_pid_file()?;
            return Ok(true);
        }
        // Process is alive — another instance is running
        anyhow::bail!(
            "RTB daemon is already running (PID {}). Stop it first with `rtb stop`.",
            pid
        );
    }

    // PID file exists but couldn't be parsed — remove it
    tracing::warn!("Found corrupt PID file. Removing.");
    remove_pid_file()?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Session restore: mark orphaned sessions as crashed
// ---------------------------------------------------------------------------

/// Scan the session store for sessions with status "running" or "suspended".
///
/// Terminal sessions cannot survive daemon restarts (PTY processes are gone),
/// so they are marked "exited".  Agent sessions may be resumable, so they are
/// marked "suspended".
pub fn restore_sessions() -> Result<()> {
    let sessions_dir = Config::rtb_dir()
        .map(|d| d.join("sessions"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/rtb/sessions"));

    // If the sessions directory doesn't exist yet, nothing to restore
    if !sessions_dir.exists() {
        return Ok(());
    }

    let store = SessionStore::new(sessions_dir)
        .context("failed to open session store for restore")?;

    let sessions = store.list().unwrap_or_default();
    let mut exited_count = 0u32;
    let mut suspended_count = 0u32;

    for mut meta in sessions {
        if meta.status == SessionStatus::Running || meta.status == SessionStatus::Suspended {
            let old_status = format!("{:?}", meta.status);

            // Terminal sessions: PTY processes cannot survive restarts
            // Agent sessions: may be resumable, mark as suspended
            let new_status = match meta.session_type {
                SessionType::Terminal => SessionStatus::Exited,
                SessionType::Agent => SessionStatus::Suspended,
            };

            meta.status = new_status.clone();
            if let Err(e) = store.update_meta(&meta.id, &meta) {
                tracing::warn!(
                    session_id = %meta.id,
                    error = %e,
                    "Failed to update orphaned session"
                );
            } else {
                tracing::info!(
                    session_id = %meta.id,
                    old_status = %old_status,
                    new_status = ?new_status,
                    session_type = ?meta.session_type,
                    "Restored orphaned session"
                );
                match meta.session_type {
                    SessionType::Terminal => exited_count += 1,
                    SessionType::Agent => suspended_count += 1,
                }
            }
        }
    }

    if exited_count > 0 || suspended_count > 0 {
        tracing::info!(
            exited = exited_count,
            suspended = suspended_count,
            "Session restore: {} terminal session(s) marked exited, {} agent session(s) marked suspended",
            exited_count,
            suspended_count,
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// API helper
// ---------------------------------------------------------------------------

/// Return the `(base_url, token)` pair for talking to a running daemon.
pub fn api_base() -> Result<(String, String)> {
    let config = Config::load().unwrap_or_default();
    let token = read_token(&config)?;
    let base = format!("http://{}:{}", config.server.host, config.server.port);
    Ok((base, token))
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Return the expanded token file path from config.
pub fn expand_token_path(config: &Config) -> String {
    expand_tilde(&config.security.token_file)
}

/// Expand a leading `~` to the current user's home directory.
fn expand_tilde(s: &str) -> String {
    if s.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string() + &s[1..];
        }
    }
    s.to_string()
}

/// Check whether a process with the given PID is still alive.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // kill(pid, 0) checks existence without sending a signal.
    unsafe { libc_kill_check(pid) }
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    // On non-Unix we conservatively assume the process is alive.
    true
}

// ---------------------------------------------------------------------------
// Thin libc wrappers – avoids pulling in the `libc` crate for two syscalls.
// ---------------------------------------------------------------------------

#[cfg(unix)]
unsafe fn libc_kill(pid: u32) {
    // SIGTERM = 15 on all Unix platforms
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe {
        kill(pid as i32, 15);
    }
}

#[cfg(unix)]
unsafe fn libc_kill_check(pid: u32) -> bool {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid as i32, 0) == 0 }
}
