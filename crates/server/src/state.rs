use rtb_core::CoreState;
use rtb_plugin_host::manager::PluginManager;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::blocklist::IpBlocklist;
use crate::rate_limit::RateLimiter;

/// Shared application state threaded through all Axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// Core subsystem: config, event bus, PTY manager, session store.
    pub core: Arc<CoreState>,
    /// Authentication token. Wrapped in `RwLock` to allow rotation without restart.
    pub token: Arc<RwLock<String>>,
    /// Per-IP rate limiter (auth, WS, GET, POST categories).
    pub rate_limiter: Arc<RateLimiter>,
    /// IP blocklist with progressive ban durations.
    pub blocklist: Arc<IpBlocklist>,
    /// Optional plugin manager. `None` when the daemon is not fully started
    /// or when accessed from contexts where PluginManager is not available.
    pub plugin_manager: Option<Arc<PluginManager>>,
    /// Current tunnel URL, updated when TunnelReady events are received.
    pub tunnel_url: Arc<RwLock<Option<String>>>,
}
