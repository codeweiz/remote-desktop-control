use rtb_core::CoreState;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state threaded through all Axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// Core subsystem: config, event bus, PTY manager, session store.
    pub core: Arc<CoreState>,
    /// Authentication token. Wrapped in `RwLock` to allow rotation without restart.
    pub token: Arc<RwLock<String>>,
}
