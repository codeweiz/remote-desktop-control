use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Application configuration, loaded from `~/.rtb/config.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub session: SessionConfig,
    pub agent: AgentConfig,
    pub notification: NotificationConfig,
    pub task_pool: TaskPoolConfig,
    pub logging: LoggingConfig,
    pub tunnel: TunnelConfig,
    pub plugins: PluginsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub shell: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub token_file: String,
    pub ip_whitelist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub max_age_days: u32,
    pub max_storage_mb: u32,
    pub session_id_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub default_provider: String,
    pub default_model: String,
    pub auto_approve_tools: bool,
    pub restart_max_attempts: u32,
    pub restart_window_secs: u64,
    pub restart_backoff_base_secs: u64,
    pub restart_backoff_max_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub channels: Vec<String>,
    pub long_running_threshold_secs: u64,
    pub sound_enabled: bool,
    #[serde(default)]
    pub rules: HashMap<String, NotificationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRule {
    pub channels: Vec<String>,
    pub min_duration: Option<String>,
    pub urgent: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskPoolConfig {
    pub max_concurrent: usize,
    pub auto_approve: bool,
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
    pub access_log: String,
    pub max_file_size_mb: u32,
    pub max_files: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TunnelConfig {
    pub provider: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginsConfig {
    pub dir: String,
    pub im_throttle_interval_ms: u64,
    pub jsonrpc_timeout_secs: u64,
    pub max_message_size_bytes: usize,
}

// ---------------------------------------------------------------------------
// Workspace config (.rtb.toml) — hot-reloadable subset
// ---------------------------------------------------------------------------

/// Workspace-level configuration loaded from `.rtb.toml` in a project directory.
/// Fields are all optional; only set values override the global config.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkspaceConfig {
    pub agent: Option<WorkspaceAgentConfig>,
    pub task_pool: Option<WorkspaceTaskPoolConfig>,
}

/// Workspace-level agent overrides.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceAgentConfig {
    pub default_provider: Option<String>,
    pub auto_approve_tools: Option<bool>,
    pub system_prompt: Option<String>,
}

/// Workspace-level task pool overrides.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceTaskPoolConfig {
    pub auto_start: Option<bool>,
    pub max_concurrent: Option<usize>,
}

// ---------------------------------------------------------------------------
// Default implementations
// ---------------------------------------------------------------------------

// Config uses #[derive(Default)] on the struct definition.

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            shell: "/bin/zsh".to_string(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            token_file: "~/.rtb/session.token".to_string(),
            ip_whitelist: Vec::new(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_age_days: 30,
            max_storage_mb: 1024,
            session_id_length: 12,
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_provider: "claude-code".to_string(),
            default_model: String::new(),
            auto_approve_tools: false,
            restart_max_attempts: 3,
            restart_window_secs: 300,
            restart_backoff_base_secs: 3,
            restart_backoff_max_secs: 30,
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            channels: vec!["web".to_string(), "desktop".to_string()],
            long_running_threshold_secs: 30,
            sound_enabled: false,
            rules: HashMap::new(),
        }
    }
}

impl Default for TaskPoolConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 1,
            auto_approve: false,
            auto_start: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            access_log: "~/.rtb/logs/access.jsonl".to_string(),
            max_file_size_mb: 10,
            max_files: 5,
        }
    }
}

// TunnelConfig uses #[derive(Default)] on the struct definition.

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            dir: "~/.rtb/plugins".to_string(),
            im_throttle_interval_ms: 5000,
            jsonrpc_timeout_secs: 30,
            max_message_size_bytes: 1_048_576,
        }
    }
}

// ---------------------------------------------------------------------------
// Config methods
// ---------------------------------------------------------------------------

impl Config {
    /// Returns the path to `~/.rtb/`.
    pub fn rtb_dir() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;
        Ok(home.join(".rtb"))
    }

    /// Default config file path: `~/.rtb/config.toml`.
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::rtb_dir()?.join("config.toml"))
    }

    /// Load config from the default path (`~/.rtb/config.toml`).
    /// Returns defaults if the file does not exist.
    /// Applies env var overrides and tilde expansion.
    ///
    /// Before loading, checks for a legacy v1 `config.json` in the same
    /// directory and migrates it to `config.toml` if present.
    pub fn load() -> Result<Self, ConfigError> {
        // Attempt v1 migration before loading
        if let Err(e) = Self::migrate_from_v1() {
            // Non-fatal: log and continue
            eprintln!("Warning: v1 config migration failed: {}", e);
        }

        let path = Self::default_path()?;
        let path_str = path
            .to_str()
            .ok_or_else(|| ConfigError::InvalidPath(path.display().to_string()))?;
        Self::load_from_path(path_str)
    }

    /// Load config from a specific path.
    /// Returns defaults if the file does not exist.
    /// Applies env var overrides and tilde expansion.
    pub fn load_from_path(path: &str) -> Result<Self, ConfigError> {
        let expanded = expand_tilde_in_str(path);
        let path = std::path::Path::new(&expanded);

        let mut cfg = if path.exists() {
            let content =
                std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e.to_string()))?;
            toml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?
        } else {
            Config::default()
        };

        cfg.apply_env_overrides();
        cfg.expand_tilde();
        Ok(cfg)
    }

    /// Save config to the default path (`~/.rtb/config.toml`).
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::default_path()?;
        let path_str = path
            .to_str()
            .ok_or_else(|| ConfigError::InvalidPath(path.display().to_string()))?;
        self.save_to_path(path_str)
    }

    /// Save config to a specific path. Creates parent directories as needed.
    pub fn save_to_path(&self, path: &str) -> Result<(), ConfigError> {
        let expanded = expand_tilde_in_str(path);
        let path = std::path::Path::new(&expanded);

        // Create parent directory with 0700 permissions if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e.to_string()))?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(0o700);
                    std::fs::set_permissions(parent, perms)
                        .map_err(|e| ConfigError::Io(e.to_string()))?;
                }
            }
        }

        let content =
            toml::to_string_pretty(self).map_err(|e| ConfigError::Serialize(e.to_string()))?;
        std::fs::write(path, content).map_err(|e| ConfigError::Io(e.to_string()))?;
        Ok(())
    }

    /// Migrate a v1 `config.json` (from the old Node.js RTB) to the
    /// current `config.toml` format.
    ///
    /// The v1 JSON format looked like:
    /// ```json
    /// {
    ///   "feishu": { "appId": "...", "appSecret": "..." },
    ///   "tunnel": { "type": "quick", "name": "..." },
    ///   "port": 3000,
    ///   "host": "0.0.0.0"
    /// }
    /// ```
    ///
    /// On success:
    /// 1. Writes a new `config.toml` with the mapped fields.
    /// 2. Renames `config.json` to `config.json.v1.bak`.
    /// 3. Prints a migration summary to stderr.
    ///
    /// Does nothing if `config.json` does not exist or `config.toml` already
    /// exists (we never overwrite an existing TOML config).
    pub fn migrate_from_v1() -> Result<(), ConfigError> {
        let rtb_dir = Self::rtb_dir()?;
        let json_path = rtb_dir.join("config.json");
        let toml_path = rtb_dir.join("config.toml");

        // Only migrate if config.json exists and config.toml does NOT
        if !json_path.exists() || toml_path.exists() {
            return Ok(());
        }

        let content =
            std::fs::read_to_string(&json_path).map_err(|e| ConfigError::Io(e.to_string()))?;

        let v1: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("v1 config.json parse error: {}", e)))?;

        let mut config = Config::default();
        let mut migrated_fields = Vec::new();

        // Map known v1 fields to the new config structure
        if let Some(port) = v1.get("port").and_then(|v| v.as_u64()) {
            config.server.port = port as u16;
            migrated_fields.push("port");
        }
        if let Some(host) = v1.get("host").and_then(|v| v.as_str()) {
            config.server.host = host.to_string();
            migrated_fields.push("host");
        }
        if let Some(shell) = v1.get("shell").and_then(|v| v.as_str()) {
            config.server.shell = shell.to_string();
            migrated_fields.push("shell");
        }

        // Tunnel config
        if let Some(tunnel) = v1.get("tunnel") {
            if let Some(tunnel_type) = tunnel.get("type").and_then(|v| v.as_str()) {
                config.tunnel.provider = tunnel_type.to_string();
                migrated_fields.push("tunnel.type -> tunnel.provider");
            }
            if let Some(name) = tunnel.get("name").and_then(|v| v.as_str()) {
                config.tunnel.domain = name.to_string();
                migrated_fields.push("tunnel.name -> tunnel.domain");
            }
        }

        // Write the new config.toml
        let toml_content =
            toml::to_string_pretty(&config).map_err(|e| ConfigError::Serialize(e.to_string()))?;

        // Ensure the directory exists
        if let Some(parent) = toml_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e.to_string()))?;
        }

        std::fs::write(&toml_path, toml_content).map_err(|e| ConfigError::Io(e.to_string()))?;

        // Rename config.json to config.json.v1.bak
        let backup_path = rtb_dir.join("config.json.v1.bak");
        std::fs::rename(&json_path, &backup_path).map_err(|e| ConfigError::Io(e.to_string()))?;

        // Print migration summary
        eprintln!("Migrated v1 config.json -> config.toml");
        if migrated_fields.is_empty() {
            eprintln!("  No known fields found; wrote defaults.");
        } else {
            eprintln!("  Migrated fields: {}", migrated_fields.join(", "));
        }
        eprintln!("  Backup saved: {}", backup_path.display());

        Ok(())
    }

    /// Apply environment variable overrides with `RTB_` prefix.
    ///
    /// For example, `RTB_SERVER_PORT=8080` overrides `server.port`.
    /// Uses `_` as separator for nested keys.
    pub fn apply_env_overrides(&mut self) {
        // Server overrides
        if let Ok(v) = std::env::var("RTB_SERVER_HOST") {
            self.server.host = v;
        }
        if let Ok(v) = std::env::var("RTB_SERVER_PORT") {
            if let Ok(p) = v.parse() {
                self.server.port = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_SERVER_SHELL") {
            self.server.shell = v;
        }

        // Security overrides
        if let Ok(v) = std::env::var("RTB_SECURITY_TOKEN_FILE") {
            self.security.token_file = v;
        }

        // Session overrides
        if let Ok(v) = std::env::var("RTB_SESSION_MAX_AGE_DAYS") {
            if let Ok(p) = v.parse() {
                self.session.max_age_days = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_SESSION_MAX_STORAGE_MB") {
            if let Ok(p) = v.parse() {
                self.session.max_storage_mb = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_SESSION_SESSION_ID_LENGTH") {
            if let Ok(p) = v.parse() {
                self.session.session_id_length = p;
            }
        }

        // Agent overrides
        if let Ok(v) = std::env::var("RTB_AGENT_DEFAULT_PROVIDER") {
            self.agent.default_provider = v;
        }
        if let Ok(v) = std::env::var("RTB_AGENT_DEFAULT_MODEL") {
            self.agent.default_model = v;
        }
        if let Ok(v) = std::env::var("RTB_AGENT_AUTO_APPROVE_TOOLS") {
            if let Ok(p) = v.parse() {
                self.agent.auto_approve_tools = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_AGENT_RESTART_MAX_ATTEMPTS") {
            if let Ok(p) = v.parse() {
                self.agent.restart_max_attempts = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_AGENT_RESTART_WINDOW_SECS") {
            if let Ok(p) = v.parse() {
                self.agent.restart_window_secs = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_AGENT_RESTART_BACKOFF_BASE_SECS") {
            if let Ok(p) = v.parse() {
                self.agent.restart_backoff_base_secs = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_AGENT_RESTART_BACKOFF_MAX_SECS") {
            if let Ok(p) = v.parse() {
                self.agent.restart_backoff_max_secs = p;
            }
        }

        // Notification overrides
        if let Ok(v) = std::env::var("RTB_NOTIFICATION_LONG_RUNNING_THRESHOLD_SECS") {
            if let Ok(p) = v.parse() {
                self.notification.long_running_threshold_secs = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_NOTIFICATION_SOUND_ENABLED") {
            if let Ok(p) = v.parse() {
                self.notification.sound_enabled = p;
            }
        }

        // Task pool overrides
        if let Ok(v) = std::env::var("RTB_TASK_POOL_MAX_CONCURRENT") {
            if let Ok(p) = v.parse() {
                self.task_pool.max_concurrent = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_TASK_POOL_AUTO_APPROVE") {
            if let Ok(p) = v.parse() {
                self.task_pool.auto_approve = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_TASK_POOL_AUTO_START") {
            if let Ok(p) = v.parse() {
                self.task_pool.auto_start = p;
            }
        }

        // Logging overrides
        if let Ok(v) = std::env::var("RTB_LOGGING_LEVEL") {
            self.logging.level = v;
        }
        if let Ok(v) = std::env::var("RTB_LOGGING_ACCESS_LOG") {
            self.logging.access_log = v;
        }
        if let Ok(v) = std::env::var("RTB_LOGGING_MAX_FILE_SIZE_MB") {
            if let Ok(p) = v.parse() {
                self.logging.max_file_size_mb = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_LOGGING_MAX_FILES") {
            if let Ok(p) = v.parse() {
                self.logging.max_files = p;
            }
        }

        // Tunnel overrides
        if let Ok(v) = std::env::var("RTB_TUNNEL_PROVIDER") {
            self.tunnel.provider = v;
        }
        if let Ok(v) = std::env::var("RTB_TUNNEL_DOMAIN") {
            self.tunnel.domain = v;
        }

        // Plugins overrides
        if let Ok(v) = std::env::var("RTB_PLUGINS_DIR") {
            self.plugins.dir = v;
        }
        if let Ok(v) = std::env::var("RTB_PLUGINS_IM_THROTTLE_INTERVAL_MS") {
            if let Ok(p) = v.parse() {
                self.plugins.im_throttle_interval_ms = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_PLUGINS_JSONRPC_TIMEOUT_SECS") {
            if let Ok(p) = v.parse() {
                self.plugins.jsonrpc_timeout_secs = p;
            }
        }
        if let Ok(v) = std::env::var("RTB_PLUGINS_MAX_MESSAGE_SIZE_BYTES") {
            if let Ok(p) = v.parse() {
                self.plugins.max_message_size_bytes = p;
            }
        }
    }

    /// Expand `~` to the actual home directory in path-like fields.
    pub fn expand_tilde(&mut self) {
        self.security.token_file = expand_tilde_in_str(&self.security.token_file);
        self.logging.access_log = expand_tilde_in_str(&self.logging.access_log);
        self.plugins.dir = expand_tilde_in_str(&self.plugins.dir);
    }

    /// Merge workspace config on top of global config.
    /// Workspace values override global where set.
    pub fn merge_workspace(&self, ws: &WorkspaceConfig) -> Config {
        let mut merged = self.clone();
        if let Some(ref agent) = ws.agent {
            if let Some(ref p) = agent.default_provider {
                merged.agent.default_provider = p.clone();
            }
            if let Some(v) = agent.auto_approve_tools {
                merged.agent.auto_approve_tools = v;
            }
        }
        if let Some(ref tp) = ws.task_pool {
            if let Some(v) = tp.max_concurrent {
                merged.task_pool.max_concurrent = v;
            }
            if let Some(v) = tp.auto_start {
                merged.task_pool.auto_start = v;
            }
        }
        merged
    }
}

/// Replace a leading `~` with the user's home directory.
fn expand_tilde_in_str(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string() + rest;
        }
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// Workspace config loading & hot reload
// ---------------------------------------------------------------------------

/// Load workspace config from `.rtb.toml` in the given directory.
/// Returns `None` if the file doesn't exist or cannot be parsed.
pub fn load_workspace_config(dir: &Path) -> Option<WorkspaceConfig> {
    let path = dir.join(".rtb.toml");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&content).ok()
}

/// Watch a config file for changes and invoke `callback` on modify/create.
///
/// Watches the parent directory of `path` (non-recursively) so that file
/// creation is also detected. The returned [`RecommendedWatcher`] must be
/// kept alive for the watch to remain active.
pub fn watch_config(
    path: PathBuf,
    callback: impl Fn() + Send + 'static,
) -> Option<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() {
                callback();
            }
        }
    })
    .ok()?;

    if let Some(parent) = path.parent() {
        watcher.watch(parent, RecursiveMode::NonRecursive).ok()?;
    }
    Some(watcher)
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("home directory not found")]
    NoHomeDir,
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("TOML parse error: {0}")]
    Parse(String),
    #[error("TOML serialize error: {0}")]
    Serialize(String),
}
