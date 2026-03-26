use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};
use tokio::sync::RwLock;

use rtb_core::config::Config;
use rtb_core::CoreState;
use rtb_plugin_host::manager::PluginManager;

// ---------------------------------------------------------------------------
// Daemon state shared across Tauri commands
// ---------------------------------------------------------------------------

pub struct DaemonState {
    /// The core state (config, event bus, PTY manager, session store, etc.).
    /// `None` until the embedded daemon has been started.
    pub core: Option<Arc<CoreState>>,
    /// The authentication token for the embedded server.
    pub token: String,
    /// The port the embedded server is listening on.
    pub port: u16,
    /// Plugin manager, if started.
    pub plugin_manager: Option<Arc<PluginManager>>,
}

pub type DaemonStateRef = Arc<RwLock<DaemonState>>;

/// Create the initial daemon state (not yet started).
pub fn create_daemon_state() -> DaemonStateRef {
    Arc::new(RwLock::new(DaemonState {
        core: None,
        token: String::new(),
        port: 3000,
        plugin_manager: None,
    }))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_status(state: State<'_, DaemonStateRef>) -> Result<serde_json::Value, String> {
    let s = state.read().await;
    Ok(serde_json::json!({
        "running": s.core.is_some(),
        "port": s.port,
        "has_token": !s.token.is_empty(),
    }))
}

#[tauri::command]
pub async fn get_config(_state: State<'_, DaemonStateRef>) -> Result<serde_json::Value, String> {
    let config = Config::load().unwrap_or_default();
    serde_json::to_value(&config).map_err(|e| format!("Failed to serialize config: {}", e))
}

#[tauri::command]
pub async fn get_sessions(state: State<'_, DaemonStateRef>) -> Result<serde_json::Value, String> {
    let s = state.read().await;
    if let Some(core) = &s.core {
        let sessions = core.pty_manager.list_sessions();
        serde_json::to_value(&sessions).map_err(|e| format!("Failed to serialize sessions: {}", e))
    } else {
        Err("Daemon not running".into())
    }
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, DaemonStateRef>,
    name: Option<String>,
    shell: Option<String>,
    cwd: Option<String>,
) -> Result<serde_json::Value, String> {
    let s = state.read().await;
    if let Some(core) = &s.core {
        let session_name = name.unwrap_or_else(|| "desktop-session".to_string());
        let cwd_path = cwd.map(PathBuf::from);
        let id = core
            .pty_manager
            .create_session(
                &session_name,
                shell.as_deref(),
                cwd_path.as_deref(),
            )
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        Ok(serde_json::json!({ "id": id }))
    } else {
        Err("Daemon not running".into())
    }
}

#[tauri::command]
pub async fn delete_session(
    state: State<'_, DaemonStateRef>,
    id: String,
) -> Result<serde_json::Value, String> {
    let s = state.read().await;
    if let Some(core) = &s.core {
        core.pty_manager
            .kill_session(&id)
            .await
            .map_err(|e| format!("Failed to delete session: {}", e))?;
        Ok(serde_json::json!({ "ok": true }))
    } else {
        Err("Daemon not running".into())
    }
}

#[tauri::command]
pub async fn start_daemon(
    app: AppHandle,
    state: State<'_, DaemonStateRef>,
) -> Result<serde_json::Value, String> {
    {
        let s = state.read().await;
        if s.core.is_some() {
            return Ok(serde_json::json!({ "status": "already_running" }));
        }
    }
    start_embedded_daemon(app)
        .await
        .map_err(|e| format!("Failed to start daemon: {}", e))?;
    Ok(serde_json::json!({ "status": "started" }))
}

#[tauri::command]
pub async fn stop_daemon(state: State<'_, DaemonStateRef>) -> Result<serde_json::Value, String> {
    let mut s = state.write().await;
    if s.core.is_none() {
        return Ok(serde_json::json!({ "status": "not_running" }));
    }

    // Stop plugins if running
    if let Some(pm) = s.plugin_manager.take() {
        pm.stop_all().await;
    }

    // Drop the core state (this will clean up PTY sessions, etc.)
    s.core = None;
    s.token = String::new();

    tracing::info!("Embedded daemon stopped");
    Ok(serde_json::json!({ "status": "stopped" }))
}

// ---------------------------------------------------------------------------
// Internal: start the embedded daemon
// ---------------------------------------------------------------------------

/// Start the embedded RTB daemon within the Tauri app.
///
/// This mirrors the logic from the CLI `start` command but runs in-process:
/// - Loads configuration from `~/.rtb/config.toml`
/// - Initialises CoreState (event bus, PTY manager, session store, etc.)
/// - Generates or loads an authentication token
/// - Starts the Axum HTTP/WebSocket server on a background task
/// - Starts the plugin manager
/// - Stores everything in the shared DaemonState
pub async fn start_embedded_daemon(app: AppHandle) -> anyhow::Result<()> {
    let config = Config::load().unwrap_or_default();
    let port = config.server.port;
    let host = config.server.host.clone();

    // Initialize core state
    let core = Arc::new(CoreState::new(config.clone())?);

    // Load persisted tasks
    if let Err(e) = core.task_pool.load().await {
        tracing::warn!(error = %e, "Failed to load task pool (continuing with empty pool)");
    }

    // Start plugin manager
    let plugins_dir = PathBuf::from(&config.plugins.dir);
    let plugin_manager = Arc::new(PluginManager::new(
        plugins_dir,
        Arc::clone(&core.event_bus),
        config.plugins.jsonrpc_timeout_secs,
    ));
    if let Err(e) = plugin_manager.start_all().await {
        tracing::warn!(error = %e, "Plugin manager start_all encountered errors (continuing)");
    }

    // Background: plugin health checks every 10 seconds
    tokio::spawn({
        let pm = Arc::clone(&plugin_manager);
        async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                pm.health_check().await;
            }
        }
    });

    // Generate or load auth token
    let token = load_or_create_token(&config)?;

    // Start the HTTP/WebSocket server on a background task
    let server_core = Arc::clone(&core);
    let server_token = token.clone();
    let server_pm = Arc::clone(&plugin_manager);
    tokio::spawn(async move {
        let blocklist = Arc::new(rtb_server::blocklist::IpBlocklist::new(Vec::new()));
        let rate_limiter = Arc::new(rtb_server::rate_limit::RateLimiter::new());

        // Background: clean up expired bans every 5 minutes
        tokio::spawn({
            let blocklist = blocklist.clone();
            async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(5 * 60));
                loop {
                    interval.tick().await;
                    blocklist.cleanup_expired();
                }
            }
        });

        let state = rtb_server::state::AppState {
            core: server_core,
            token: Arc::new(tokio::sync::RwLock::new(server_token)),
            rate_limiter,
            blocklist,
            plugin_manager: Some(server_pm),
        };

        let app = rtb_server::router::create_router(state);
        let addr = format!("{}:{}", host, port);

        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                tracing::info!("RTB embedded server listening on {}", addr);
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!("Embedded server error: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind embedded server to {}: {}", addr, e);
            }
        }
    });

    // Background: record notifications in the store
    {
        let notification_store = Arc::clone(&core.notification_store);
        let mut control_rx = core.event_bus.subscribe_control();
        tokio::spawn(async move {
            loop {
                match control_rx.recv().await {
                    Ok(event) => {
                        if let rtb_core::events::ControlEvent::NotificationTriggered {
                            session_id,
                            trigger_type,
                            summary,
                            urgent,
                        } = event.as_ref()
                        {
                            notification_store.push(
                                session_id.clone(),
                                trigger_type.clone(),
                                summary.clone(),
                                *urgent,
                            );
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "notification store listener lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // Store state in the Tauri managed state
    let daemon_state: DaemonStateRef = app.state::<DaemonStateRef>().inner().clone();
    {
        let mut s = daemon_state.write().await;
        s.core = Some(core);
        s.token = token;
        s.port = port;
        s.plugin_manager = Some(plugin_manager);
    }

    tracing::info!("Embedded daemon started on port {}", port);
    Ok(())
}

// ---------------------------------------------------------------------------
// Token helpers (adapted from CLI daemon module)
// ---------------------------------------------------------------------------

/// Load or create a token, similar to the CLI's daemon::load_or_create_token.
fn load_or_create_token(config: &Config) -> anyhow::Result<String> {
    let token_path = expand_tilde(&config.security.token_file);

    // Try to read an existing token
    if let Ok(token) = std::fs::read_to_string(&token_path) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Generate a new 256-bit random token (64 hex chars)
    let bytes: [u8; 32] = rand_bytes();
    let token = hex_encode(&bytes);

    // Create parent dir if needed
    if let Some(parent) = std::path::Path::new(&token_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&token_path, &token)?;

    // Restrict token file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
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

/// Generate 32 random bytes using a simple approach that avoids
/// pulling in the full `rand` crate. Falls back to reading /dev/urandom
/// on Unix or uses std::collections::hash_map::RandomState on all platforms.
fn rand_bytes() -> [u8; 32] {
    let mut buf = [0u8; 32];

    #[cfg(unix)]
    {
        if let Ok(bytes) = std::fs::read("/dev/urandom") {
            for (i, b) in bytes.iter().take(32).enumerate() {
                buf[i] = *b;
            }
            return buf;
        }
    }

    // Fallback: use hash-based randomness from multiple RandomState instances
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    for chunk in buf.chunks_mut(8) {
        let s = RandomState::new();
        let val = s.build_hasher().finish().to_le_bytes();
        for (i, b) in chunk.iter_mut().enumerate() {
            if i < val.len() {
                *b = val[i];
            }
        }
    }
    buf
}

/// Simple hex encoding without pulling in the `hex` crate.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
