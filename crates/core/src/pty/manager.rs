use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use tracing::{debug, info};

use crate::config::Config;
use crate::event_bus::EventBus;
use crate::events::{ControlEvent, SessionType};

use super::session::{PtySession, PtySessionInfo};

/// Manages multiple PTY sessions.
///
/// Provides CRUD operations for terminal sessions, delegating the actual
/// PTY handling to `PtySession`. Publishes lifecycle events
/// (`SessionCreated`, `SessionDeleted`) to the EventBus.
pub struct PtyManager {
    sessions: DashMap<String, Arc<PtySession>>,
    event_bus: Arc<EventBus>,
    config: Arc<Config>,
}

impl PtyManager {
    /// Create a new PTY manager.
    pub fn new(event_bus: Arc<EventBus>, config: Arc<Config>) -> Self {
        Self {
            sessions: DashMap::new(),
            event_bus,
            config,
        }
    }

    /// Create a new PTY session.
    ///
    /// Spawns a PTY with the given shell (or the configured default),
    /// starts a background output reader, and publishes a
    /// `ControlEvent::SessionCreated` event.
    ///
    /// Returns the generated session ID.
    pub async fn create_session(
        &self,
        name: &str,
        shell: Option<&str>,
        cwd: Option<&Path>,
    ) -> Result<String> {
        let id_length = self.config.session.session_id_length;
        let session_id = nanoid::nanoid!(id_length);

        let shell = shell.unwrap_or(&self.config.server.shell);
        let buffer_capacity = self.config.session.buffer_size;

        let session = PtySession::spawn(
            session_id.clone(),
            name.to_string(),
            shell,
            cwd,
            self.event_bus.clone(),
            buffer_capacity,
        )?;

        self.sessions.insert(session_id.clone(), session);

        info!(session_id = %session_id, name = %name, shell = %shell, "created PTY session");

        self.event_bus.publish_control(ControlEvent::SessionCreated {
            session_id: session_id.clone(),
            session_type: SessionType::Terminal,
        });

        Ok(session_id)
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: &str) -> Option<Arc<PtySession>> {
        self.sessions.get(id).map(|entry| entry.value().clone())
    }

    /// List all active sessions as lightweight info structs.
    pub fn list_sessions(&self) -> Vec<PtySessionInfo> {
        self.sessions
            .iter()
            .map(|entry| entry.value().info())
            .collect()
    }

    /// Write input data to the stdin of the specified session.
    pub fn write_input(&self, id: &str, data: &[u8]) -> Result<()> {
        let session = self
            .sessions
            .get(id)
            .ok_or_else(|| anyhow!("session not found: {}", id))?;
        session.write_input(data)
    }

    /// Resize the terminal of the specified session.
    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let session = self
            .sessions
            .get(id)
            .ok_or_else(|| anyhow!("session not found: {}", id))?;
        session.resize(cols, rows)?;
        debug!(session_id = %id, cols, rows, "resized PTY session");
        Ok(())
    }

    /// Kill a session and remove it from management.
    ///
    /// Kills the child process, removes the session from the internal map,
    /// publishes a `ControlEvent::SessionDeleted` event, and cleans up
    /// the EventBus session channels.
    pub async fn kill_session(&self, id: &str) -> Result<()> {
        let (_, session) = self
            .sessions
            .remove(id)
            .ok_or_else(|| anyhow!("session not found: {}", id))?;

        // Kill the child process. Ignore errors if already exited.
        if session.is_running() {
            if let Err(e) = session.kill() {
                debug!(session_id = %id, error = %e, "error killing PTY (may have already exited)");
            }
        }

        info!(session_id = %id, "killed PTY session");

        self.event_bus.publish_control(ControlEvent::SessionDeleted {
            session_id: id.to_string(),
        });

        self.event_bus.remove_session(id);

        Ok(())
    }

    /// Return the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}
