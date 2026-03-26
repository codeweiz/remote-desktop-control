use rtb_core::CoreState;
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
}
