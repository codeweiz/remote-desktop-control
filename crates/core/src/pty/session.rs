use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};
use chrono::{DateTime, Utc};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tracing::{debug, error, warn};

use crate::event_bus::EventBus;
use crate::events::DataEvent;

use super::buffer::RingBuffer;

/// Default output coalescing window in milliseconds.
const DEFAULT_COALESCE_MS: u64 = 100;

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

/// A single PTY session wrapping a pseudo-terminal process.
///
/// On creation, the session spawns a shell process and starts a background
/// task that reads stdout and publishes output to the EventBus. Output is
/// also stored in a ring buffer for replay.
pub struct PtySession {
    id: String,
    name: String,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    buffer: Arc<RingBuffer>,
    seq: Arc<AtomicU64>,
    status: Arc<RwLock<PtyStatus>>,
    created_at: DateTime<Utc>,
    cwd: PathBuf,
    shell: String,
}

impl PtySession {
    /// Spawn a new PTY session.
    ///
    /// This opens a pseudo-terminal, spawns the given shell, and starts a
    /// tokio task that reads output from the PTY and publishes it to the
    /// EventBus and ring buffer.
    pub fn spawn(
        id: String,
        name: String,
        shell: &str,
        cwd: Option<&std::path::Path>,
        event_bus: Arc<EventBus>,
        buffer_capacity: usize,
    ) -> anyhow::Result<Arc<Self>> {
        Self::spawn_with_coalesce(id, name, shell, cwd, event_bus, buffer_capacity, DEFAULT_COALESCE_MS)
    }

    /// Spawn a new PTY session with a configurable output coalescing window.
    ///
    /// `coalesce_ms` controls how long (in milliseconds) to buffer PTY output
    /// before flushing it as a single event. Set to 0 to disable coalescing.
    pub fn spawn_with_coalesce(
        id: String,
        name: String,
        shell: &str,
        cwd: Option<&std::path::Path>,
        event_bus: Arc<EventBus>,
        buffer_capacity: usize,
        coalesce_ms: u64,
    ) -> anyhow::Result<Arc<Self>> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(size)?;

        let mut cmd = CommandBuilder::new(shell);
        let working_dir = cwd
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
            });
        cmd.cwd(&working_dir);

        let mut child = pair.slave.spawn_command(cmd)?;
        let killer = child.clone_killer();
        let writer = pair.master.take_writer()?;
        let reader = pair.master.try_clone_reader()?;

        let buffer = Arc::new(RingBuffer::new(buffer_capacity));
        let seq = Arc::new(AtomicU64::new(0));
        let status = Arc::new(RwLock::new(PtyStatus::Running));

        let session = Arc::new(Self {
            id: id.clone(),
            name,
            killer: Mutex::new(killer),
            writer: Mutex::new(writer),
            master: Mutex::new(pair.master),
            buffer: buffer.clone(),
            seq: seq.clone(),
            status: status.clone(),
            created_at: Utc::now(),
            cwd: working_dir,
            shell: shell.to_string(),
        });

        // Start background reader task
        let session_id = id.clone();
        let reader_buffer = buffer;
        let reader_seq = seq;
        let reader_status = status;
        let reader_event_bus = event_bus;

        tokio::task::spawn_blocking(move || {
            Self::reader_loop(
                session_id,
                reader,
                reader_buffer,
                reader_seq,
                reader_event_bus,
                coalesce_ms,
            );
        });

        // Start background task to detect child exit
        let waiter_session_id = id;
        let waiter_status = reader_status;
        tokio::task::spawn_blocking(move || {
            let exit_status = child.wait();
            let exit_code = match exit_status {
                Ok(status) => status.exit_code() as i32,
                Err(e) => {
                    error!(session_id = %waiter_session_id, error = %e, "error waiting for child");
                    -1
                }
            };

            debug!(session_id = %waiter_session_id, exit_code, "PTY child exited");
            if let Ok(mut s) = waiter_status.write() {
                *s = PtyStatus::Exited(exit_code);
            }
        });

        Ok(session)
    }

    /// Background reader loop that reads from the PTY and publishes output.
    ///
    /// When `coalesce_ms > 0`, incoming data is buffered and flushed either
    /// when no more data arrives within the coalescing window or when the
    /// internal read buffer is full. This reduces the number of events and
    /// WebSocket messages for high-throughput scenarios (e.g., `find / -type f`).
    fn reader_loop(
        session_id: String,
        mut reader: Box<dyn std::io::Read + Send>,
        buffer: Arc<RingBuffer>,
        seq: Arc<AtomicU64>,
        event_bus: Arc<EventBus>,
        coalesce_ms: u64,
    ) {
        let coalesce_duration = Duration::from_millis(coalesce_ms);
        let mut read_buf = [0u8; 4096];

        // When coalescing is disabled, use the original direct-publish path
        if coalesce_ms == 0 {
            loop {
                match reader.read(&mut read_buf) {
                    Ok(0) => {
                        debug!(session_id = %session_id, "PTY reader got EOF");
                        break;
                    }
                    Ok(n) => {
                        let current_seq = seq.fetch_add(1, Ordering::SeqCst) + 1;
                        let data = Bytes::copy_from_slice(&read_buf[..n]);
                        buffer.push(current_seq, data.clone());
                        let event = DataEvent::PtyOutput {
                            seq: current_seq,
                            data,
                        };
                        let sid = session_id.clone();
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            let eb = event_bus.clone();
                            handle.spawn(async move {
                                eb.publish_data(&sid, event).await;
                            });
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::Other
                            || e.raw_os_error() == Some(libc::EIO)
                        {
                            debug!(session_id = %session_id, "PTY reader got EIO (child likely exited)");
                        } else {
                            warn!(session_id = %session_id, error = %e, "PTY reader error");
                        }
                        break;
                    }
                }
            }
            return;
        }

        // Coalescing path: buffer incoming data and flush when the coalescing
        // window elapses or the buffer grows large. Since read() is blocking,
        // we check elapsed time after each read returns.
        let mut coalesce_buf = BytesMut::new();
        let mut first_data_time: Option<Instant> = None;

        loop {
            match reader.read(&mut read_buf) {
                Ok(0) => {
                    // EOF — flush any remaining buffered data
                    if !coalesce_buf.is_empty() {
                        Self::flush_coalesce_buf(
                            &session_id,
                            &mut coalesce_buf,
                            &buffer,
                            &seq,
                            &event_bus,
                        );
                    }
                    debug!(session_id = %session_id, "PTY reader got EOF");
                    break;
                }
                Ok(n) => {
                    coalesce_buf.extend_from_slice(&read_buf[..n]);
                    if first_data_time.is_none() {
                        first_data_time = Some(Instant::now());
                    }

                    // Flush immediately if the buffer has grown large (32KB)
                    // to prevent excessive memory use.
                    let should_flush = coalesce_buf.len() >= 32 * 1024
                        || first_data_time
                            .map(|t| t.elapsed() >= coalesce_duration)
                            .unwrap_or(false);

                    if should_flush && !coalesce_buf.is_empty() {
                        Self::flush_coalesce_buf(
                            &session_id,
                            &mut coalesce_buf,
                            &buffer,
                            &seq,
                            &event_bus,
                        );
                        first_data_time = None;
                    }
                }
                Err(e) => {
                    // Flush any remaining buffered data before exiting
                    if !coalesce_buf.is_empty() {
                        Self::flush_coalesce_buf(
                            &session_id,
                            &mut coalesce_buf,
                            &buffer,
                            &seq,
                            &event_bus,
                        );
                    }
                    if e.kind() == std::io::ErrorKind::Other
                        || e.raw_os_error() == Some(libc::EIO)
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

    /// Flush the coalescing buffer as a single PtyOutput event.
    fn flush_coalesce_buf(
        session_id: &str,
        coalesce_buf: &mut BytesMut,
        ring_buffer: &Arc<RingBuffer>,
        seq: &Arc<AtomicU64>,
        event_bus: &Arc<EventBus>,
    ) {
        let current_seq = seq.fetch_add(1, Ordering::SeqCst) + 1;
        let data = coalesce_buf.split().freeze();
        ring_buffer.push(current_seq, data.clone());
        let event = DataEvent::PtyOutput {
            seq: current_seq,
            data,
        };
        let sid = session_id.to_string();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let eb = event_bus.clone();
            handle.spawn(async move {
                eb.publish_data(&sid, event).await;
            });
        }
    }

    /// Session ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Session display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Shell path.
    pub fn shell(&self) -> &str {
        &self.shell
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

    /// Reference to the output ring buffer.
    pub fn buffer(&self) -> &RingBuffer {
        &self.buffer
    }

    /// Current sequence number (last assigned).
    pub fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }

    /// Write data to the PTY stdin.
    pub fn write_input(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Resize the PTY terminal.
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        let master = self.master.lock().unwrap();
        master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Kill the child process.
    pub fn kill(&self) -> anyhow::Result<()> {
        let mut killer = self.killer.lock().unwrap();
        killer.kill()?;
        Ok(())
    }

    /// Build a lightweight info struct for listing.
    pub fn info(&self) -> PtySessionInfo {
        PtySessionInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            status: self.status(),
            created_at: self.created_at,
            shell: self.shell.clone(),
            cwd: self.cwd.clone(),
        }
    }
}
