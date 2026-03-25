// rtb-core: Core library for RTB 2.0
//
// Provides shared types, configuration management, PTY handling,
// session management, and foundational abstractions used by all
// other crates in the workspace.

pub mod config;
pub mod event_bus;
pub mod events;
pub mod pty;
pub mod session;

use std::sync::Arc;

/// Central application state owning all core components.
pub struct CoreState {
    pub config: Arc<config::Config>,
    pub event_bus: Arc<event_bus::EventBus>,
    pub pty_manager: Arc<pty::manager::PtyManager>,
    pub session_store: Arc<session::store::SessionStore>,
}

impl CoreState {
    /// Initialize all core components from config.
    pub fn new(config: config::Config) -> anyhow::Result<Self> {
        let config = Arc::new(config);
        let event_bus = Arc::new(event_bus::EventBus::new());

        let sessions_dir = config::Config::rtb_dir()
            .map(|d| d.join("sessions"))
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/rtb/sessions"));
        let session_store = Arc::new(session::store::SessionStore::new(sessions_dir)?);

        let pty_manager = Arc::new(pty::manager::PtyManager::new(
            Arc::clone(&event_bus),
            Arc::clone(&config),
        ));

        Ok(Self {
            config,
            event_bus,
            pty_manager,
            session_store,
        })
    }
}
