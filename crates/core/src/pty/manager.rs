use std::path::Path;
use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use tracing::{debug, info};

use crate::config::Config;
use crate::event_bus::EventBus;
use crate::events::{ControlEvent, DataEvent, SessionType};
use crate::notification::detector::Detector;
use crate::notification::router::NotificationRouter;

use super::session::{PtySession, PtySessionInfo};

/// Manages multiple PTY sessions.
///
/// Provides CRUD operations for terminal sessions, delegating the actual
/// PTY handling to `PtySession`. Publishes lifecycle events
/// (`SessionCreated`, `SessionDeleted`) to the EventBus.
/// Also spawns a notification detector task for each session.
pub struct PtyManager {
    sessions: DashMap<String, Arc<PtySession>>,
    event_bus: Arc<EventBus>,
    config: Arc<Config>,
    notification_router: OnceLock<Arc<NotificationRouter>>,
}

impl PtyManager {
    /// Create a new PTY manager.
    pub fn new(event_bus: Arc<EventBus>, config: Arc<Config>) -> Self {
        Self {
            sessions: DashMap::new(),
            event_bus,
            config,
            notification_router: OnceLock::new(),
        }
    }

    /// Set the notification router. Called once after CoreState is fully initialized.
    pub fn set_notification_router(&self, router: Arc<NotificationRouter>) {
        let _ = self.notification_router.set(router);
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

        let coalesce_ms = self.config.session.output_coalesce_ms;
        let session = PtySession::spawn_with_coalesce(
            session_id.clone(),
            name.to_string(),
            shell,
            cwd,
            self.event_bus.clone(),
            buffer_capacity,
            coalesce_ms,
        )?;

        self.sessions.insert(session_id.clone(), session);

        info!(session_id = %session_id, name = %name, shell = %shell, "created PTY session");

        self.event_bus.publish_control(ControlEvent::SessionCreated {
            session_id: session_id.clone(),
            session_type: SessionType::Terminal,
        });

        // Spawn a notification detector task for this session
        self.start_detector_task(&session_id);

        Ok(session_id)
    }

    /// Spawn a background task that subscribes to the session's data events,
    /// feeds output to the three-layer detector, and routes any triggers through
    /// the NotificationRouter so that WS clients and IM bridge receive them.
    fn start_detector_task(&self, session_id: &str) {
        let router = match self.notification_router.get() {
            Some(r) => Arc::clone(r),
            None => {
                debug!(session_id = %session_id, "no notification router set, skipping detector");
                return;
            }
        };

        let mut data_rx = self.event_bus.create_data_subscriber(session_id);
        let sid = session_id.to_string();
        let silence_threshold = self.config.notification.long_running_threshold_secs;
        // Use 2x silence threshold as long-running threshold
        let long_running_threshold = silence_threshold.saturating_mul(2).max(60);

        tokio::spawn(async move {
            let mut detector = Detector::new(None, silence_threshold, long_running_threshold);
            let mut periodic_interval = tokio::time::interval(
                tokio::time::Duration::from_secs(silence_threshold),
            );
            // Skip the first immediate tick
            periodic_interval.tick().await;

            info!(session_id = %sid, "notification detector started");

            loop {
                tokio::select! {
                    event = data_rx.recv() => {
                        match event {
                            Some(DataEvent::PtyOutput { data, .. }) => {
                                let text = String::from_utf8_lossy(&data);
                                let triggers = detector.process_output(&text);
                                if !triggers.is_empty() {
                                    debug!(
                                        session_id = %sid,
                                        count = triggers.len(),
                                        "detector fired triggers"
                                    );
                                    router.route(&sid, &triggers);
                                }
                            }
                            Some(DataEvent::PtyExited { exit_code }) => {
                                // Fire a process-exited signal via the detector
                                let signal = detector.process_monitor.process_exited(
                                    exit_code, None, 0.0,
                                );
                                let triggers = crate::notification::detector::fuse_signals(&[signal]);
                                if !triggers.is_empty() {
                                    router.route(&sid, &triggers);
                                }
                                debug!(session_id = %sid, exit_code, "detector stopping (PTY exited)");
                                break;
                            }
                            None => {
                                debug!(session_id = %sid, "data channel closed, detector stopping");
                                break;
                            }
                            _ => {} // ignore resize and other events
                        }
                    }
                    _ = periodic_interval.tick() => {
                        // Periodic check for stalls / long-running detection
                        let triggers = detector.periodic_check();
                        if !triggers.is_empty() {
                            debug!(
                                session_id = %sid,
                                count = triggers.len(),
                                "periodic detector fired triggers"
                            );
                            router.route(&sid, &triggers);
                        }
                    }
                }
            }
        });
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
