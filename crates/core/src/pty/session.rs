use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::{broadcast, watch};
use tracing::{debug, error, warn};

use super::osc::OscColorResponder;
use super::tmux;

/// Status of a PTY session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum PtyStatus {
    Running,
    Exited(i32),
}

/// Lightweight info struct returned by `PtyManager::list_sessions`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PtySessionInfo {
    pub id: String,
    pub name: String,
    pub status: PtyStatus,
    pub created_at: DateTime<Utc>,
    pub shell: String,
    pub cwd: PathBuf,
}

/// A single PTY session backed by a tmux session.
///
/// On creation, the session creates a tmux session, opens a PTY pair,
/// and attaches to the tmux session via `tmux attach`. A background
/// thread reads output and broadcasts it to all subscribers.
pub struct PtySession {
    id: String,
    name: String,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    live_tx: broadcast::Sender<Bytes>,
    status: Arc<RwLock<PtyStatus>>,
    status_tx: watch::Sender<PtyStatus>,
    _status_rx: watch::Receiver<PtyStatus>,
    created_at: DateTime<Utc>,
    cwd: PathBuf,
}

impl PtySession {
    /// Spawn a new tmux-backed PTY session.
    ///
    /// 1. Creates a detached tmux session named `rtb-{id}`.
    /// 2. Opens a local PTY pair and spawns `tmux attach -d -t rtb-{id}`.
    /// 3. Starts a reader thread that broadcasts output.
    /// 4. Starts a waiter thread that detects child exit.
    pub fn spawn(
        id: String,
        name: String,
        cwd: Option<&std::path::Path>,
    ) -> anyhow::Result<Arc<Self>> {
        let working_dir = cwd
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

        // 1. Create the detached tmux session
        tmux::new_session(&id, &working_dir)?;

        // 2. Open PTY pair
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size)?;

        // 3. Spawn `tmux attach -d -t rtb-{id}`
        let tmux_session_name = tmux::session_name(&id);
        let mut cmd = CommandBuilder::new("tmux");
        cmd.args(["attach", "-d", "-t", &tmux_session_name]);
        cmd.cwd(&working_dir);

        let mut child = pair.slave.spawn_command(cmd)?;

        // 4. CRITICAL: Drop the slave PTY handle after spawning.
        // The slave must be closed in the parent so that the master reader
        // can properly receive data from the child.
        drop(pair.slave);

        let killer = child.clone_killer();
        let writer: Box<dyn Write + Send> = pair.master.take_writer()?;
        let writer = Arc::new(Mutex::new(writer));
        let reader = pair.master.try_clone_reader()?;

        let status = Arc::new(RwLock::new(PtyStatus::Running));
        let (live_tx, _) = broadcast::channel::<Bytes>(256);
        let (status_tx, status_rx) = watch::channel(PtyStatus::Running);

        let session = Arc::new(Self {
            id: id.clone(),
            name,
            writer: writer.clone(),
            master: Mutex::new(pair.master),
            live_tx: live_tx.clone(),
            status: status.clone(),
            status_tx: status_tx.clone(),
            _status_rx: status_rx,
            created_at: Utc::now(),
            cwd: working_dir,
        });

        // Start reader thread (std::thread, NOT tokio::task::spawn_blocking)
        let reader_session_id = id.clone();
        let reader_writer = writer;
        let reader_live_tx = live_tx;
        std::thread::Builder::new()
            .name(format!("pty-reader-{}", &id))
            .spawn(move || {
                Self::reader_loop(reader_session_id, reader, reader_writer, reader_live_tx);
            })?;

        // Start waiter thread that detects child exit
        let waiter_session_id = id;
        let waiter_status = status;
        let waiter_status_tx = status_tx;
        std::thread::Builder::new()
            .name(format!("pty-waiter-{}", &waiter_session_id))
            .spawn(move || {
                let exit_status = child.wait();
                let exit_code = match exit_status {
                    Ok(s) => s.exit_code() as i32,
                    Err(e) => {
                        error!(session_id = %waiter_session_id, error = %e, "error waiting for child");
                        -1
                    }
                };

                debug!(session_id = %waiter_session_id, exit_code, "PTY child exited");

                // Update RwLock
                if let Ok(mut s) = waiter_status.write() {
                    *s = PtyStatus::Exited(exit_code);
                }

                // Update watch channel
                let _ = waiter_status_tx.send(PtyStatus::Exited(exit_code));

                // Drop the killer — child is already dead
                drop(killer);
            })?;

        Ok(session)
    }

    /// Background reader loop that reads PTY output and broadcasts it.
    fn reader_loop(
        session_id: String,
        mut reader: Box<dyn std::io::Read + Send>,
        writer_arc: Arc<Mutex<Box<dyn Write + Send>>>,
        live_tx: broadcast::Sender<Bytes>,
    ) {
        let mut osc = OscColorResponder::new_dark_theme();
        osc.set_writer(writer_arc);

        let mut buf = [0u8; 4096];

        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    debug!(session_id = %session_id, "PTY reader got EOF");
                    break;
                }
                Ok(n) => {
                    let chunk = &buf[..n];
                    // Let OSC responder inspect (and possibly respond to) the data
                    let _data = osc.intercept(chunk);
                    let data = Bytes::copy_from_slice(chunk);
                    // Broadcast to all subscribers; ignore error (no receivers)
                    let _ = live_tx.send(data);
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::Other || e.raw_os_error() == Some(libc::EIO)
                    {
                        debug!(session_id = %session_id, "PTY reader got EIO (child likely exited)");
                    } else {
                        warn!(session_id = %session_id, error = %e, "PTY reader error");
                    }
                    break;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Session ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Session display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Working directory.
    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    /// Creation time.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Current status.
    pub fn status(&self) -> PtyStatus {
        self.status.read().unwrap().clone()
    }

    /// Whether the session is still running.
    pub fn is_running(&self) -> bool {
        matches!(*self.status.read().unwrap(), PtyStatus::Running)
    }

    /// Subscribe to live output (broadcast channel).
    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.live_tx.subscribe()
    }

    /// Subscribe to status changes (watch channel).
    pub fn subscribe_status(&self) -> watch::Receiver<PtyStatus> {
        self.status_tx.subscribe()
    }

    /// Write data to the PTY stdin.
    pub fn write_input(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Resize the terminal.
    ///
    /// Sends the resize to both tmux (so the tmux pane adapts) and the
    /// local PTY master (so the attached `tmux attach` process knows).
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        tmux::resize_pane(&self.id, cols, rows)?;
        let master = self.master.lock().unwrap();
        master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Kill the session.
    ///
    /// Destroys the backing tmux session. The `tmux attach` child process
    /// will exit on its own once the session is gone.
    pub fn kill(&self) -> anyhow::Result<()> {
        tmux::kill_session(&self.id)?;
        Ok(())
    }

    /// Build a lightweight info struct for listing.
    pub fn info(&self) -> PtySessionInfo {
        PtySessionInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            status: self.status(),
            created_at: self.created_at,
            shell: "tmux".to_string(),
            cwd: self.cwd.clone(),
        }
    }
}
