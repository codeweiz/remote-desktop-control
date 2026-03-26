use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Utc;
use tracing::{debug, warn};

use super::types::{SessionEvent, SessionMeta};

/// Sparse index mapping every Nth event's sequence number to its byte offset
/// in the events.jsonl file. Used to accelerate `read_events_since()` on
/// large event files by seeking close to the target instead of scanning from
/// the beginning.
const INDEX_INTERVAL: usize = 1000;

/// A single entry in the sparse event index.
#[derive(Debug, Clone, Copy)]
struct IndexEntry {
    seq: u64,
    offset: u64,
}

/// Sparse seq-to-offset index for a single session's events.jsonl file.
///
/// Built lazily on first access by scanning the file once. Every
/// `INDEX_INTERVAL` events, the (seq, byte_offset) pair is recorded.
#[derive(Debug)]
struct EventIndex {
    entries: Vec<IndexEntry>,
    /// Number of events scanned when the index was built.
    events_scanned: usize,
}

impl EventIndex {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            events_scanned: 0,
        }
    }

    /// Build (or rebuild) the index by scanning the events file.
    fn build(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let file = fs::File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut index = EventIndex::new();
        let mut line = String::new();
        let mut offset: u64 = 0;
        let mut count: usize = 0;

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                break; // EOF
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                offset += bytes_read as u64;
                continue;
            }
            if let Ok(evt) = serde_json::from_str::<SessionEvent>(trimmed) {
                if count % INDEX_INTERVAL == 0 {
                    index.entries.push(IndexEntry {
                        seq: evt.seq,
                        offset,
                    });
                }
                count += 1;
            }
            offset += bytes_read as u64;
        }
        index.events_scanned = count;
        Ok(index)
    }

    /// Find the byte offset closest to (but not after) the given seq.
    /// Returns 0 if no index entry precedes `target_seq`.
    fn closest_offset_for(&self, target_seq: u64) -> u64 {
        if self.entries.is_empty() {
            return 0;
        }
        // Binary search for the last entry with seq <= target_seq
        let pos = self
            .entries
            .partition_point(|e| e.seq <= target_seq);
        if pos == 0 {
            return 0;
        }
        self.entries[pos - 1].offset
    }
}

/// Errors from session store operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("session not found: {0}")]
    NotFound(String),
}

/// Filesystem-backed session store.
///
/// Directory layout per session:
/// ```text
/// {base_dir}/
///   {session_id}/
///     meta.json
///     events.jsonl
/// ```
pub struct SessionStore {
    base_dir: PathBuf,
    /// Per-session sparse event index cache. Built lazily on first
    /// `read_events_since()` call for each session.
    index_cache: Mutex<HashMap<String, EventIndex>>,
}

impl SessionStore {
    /// Create a new session store rooted at `base_dir`.
    /// Creates the directory if it does not exist.
    pub fn new(base_dir: PathBuf) -> Result<Self, SessionStoreError> {
        fs::create_dir_all(&base_dir)?;
        Ok(Self {
            base_dir,
            index_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Path to a session's directory.
    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(session_id)
    }

    /// Path to a session's meta.json.
    fn meta_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("meta.json")
    }

    /// Path to a session's events.jsonl.
    fn events_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("events.jsonl")
    }

    /// Create a new session: write its directory and meta.json.
    pub fn create(&self, meta: &SessionMeta) -> Result<(), SessionStoreError> {
        let dir = self.session_dir(&meta.id);
        fs::create_dir_all(&dir)?;
        self.write_meta_atomic(&meta.id, meta)?;
        Ok(())
    }

    /// Read and parse a session's meta.json.
    pub fn get_meta(&self, session_id: &str) -> Result<SessionMeta, SessionStoreError> {
        let path = self.meta_path(session_id);
        if !path.exists() {
            return Err(SessionStoreError::NotFound(session_id.to_string()));
        }
        let content = fs::read_to_string(&path)?;
        let meta: SessionMeta = serde_json::from_str(&content)?;
        Ok(meta)
    }

    /// Overwrite a session's meta.json atomically (write tmp, then rename).
    pub fn update_meta(
        &self,
        session_id: &str,
        meta: &SessionMeta,
    ) -> Result<(), SessionStoreError> {
        let dir = self.session_dir(session_id);
        if !dir.exists() {
            return Err(SessionStoreError::NotFound(session_id.to_string()));
        }
        self.write_meta_atomic(session_id, meta)?;
        Ok(())
    }

    /// Delete an entire session directory.
    pub fn delete(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let dir = self.session_dir(session_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    /// List all sessions by scanning subdirectories and reading each meta.json.
    /// Skips sessions with missing or corrupt meta.json.
    pub fn list(&self) -> Result<Vec<SessionMeta>, SessionStoreError> {
        let mut sessions = Vec::new();
        let entries = fs::read_dir(&self.base_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let meta_path = path.join("meta.json");
            if !meta_path.exists() {
                continue;
            }
            match fs::read_to_string(&meta_path)
                .map_err(SessionStoreError::from)
                .and_then(|c| serde_json::from_str::<SessionMeta>(&c).map_err(Into::into))
            {
                Ok(meta) => sessions.push(meta),
                Err(e) => {
                    warn!(
                        path = %meta_path.display(),
                        error = %e,
                        "skipping session with corrupt meta.json"
                    );
                }
            }
        }
        Ok(sessions)
    }

    /// Append a single event as a JSON line to events.jsonl.
    pub fn append_event(
        &self,
        session_id: &str,
        event: &SessionEvent,
    ) -> Result<(), SessionStoreError> {
        let path = self.events_path(session_id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let line = serde_json::to_string(event)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    /// Read events with seq > since_seq.
    ///
    /// Uses a sparse in-memory index (built on first access) to seek close
    /// to the target sequence number instead of scanning from the start of
    /// the file. For small files the overhead is negligible; for large files
    /// this avoids reading potentially millions of preceding lines.
    pub fn read_events_since(
        &self,
        session_id: &str,
        since_seq: u64,
    ) -> Result<Vec<SessionEvent>, SessionStoreError> {
        let path = self.events_path(session_id);
        if !path.exists() {
            return Ok(Vec::new());
        }

        // Build or retrieve the cached index
        let start_offset = {
            let mut cache = self.index_cache.lock().unwrap();
            let index = cache.entry(session_id.to_string()).or_insert_with(|| {
                match EventIndex::build(&path) {
                    Ok(idx) => {
                        debug!(
                            session_id = session_id,
                            entries = idx.entries.len(),
                            events = idx.events_scanned,
                            "built sparse event index"
                        );
                        idx
                    }
                    Err(e) => {
                        warn!(
                            session_id = session_id,
                            error = %e,
                            "failed to build event index, falling back to full scan"
                        );
                        EventIndex::new()
                    }
                }
            });
            index.closest_offset_for(since_seq)
        };

        // Open the file and seek to the nearest indexed position
        let file = fs::File::open(&path)?;
        let mut reader = BufReader::new(file);
        if start_offset > 0 {
            reader.seek(SeekFrom::Start(start_offset))?;
        }

        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionEvent>(&line) {
                Ok(evt) => {
                    if evt.seq > since_seq {
                        events.push(evt);
                    }
                }
                Err(e) => {
                    warn!(
                        session_id = session_id,
                        error = %e,
                        "skipping malformed event line"
                    );
                }
            }
        }
        Ok(events)
    }

    /// Read all events from events.jsonl.
    /// Skips malformed lines with a warning.
    pub fn read_all_events(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionEvent>, SessionStoreError> {
        self.read_events_raw(session_id)
    }

    /// Clean up old sessions.
    ///
    /// 1. Delete sessions whose `last_active` is older than `max_age_days`.
    /// 2. If total storage exceeds `max_storage_mb`, delete oldest sessions
    ///    (by `last_active`) until under the limit.
    ///
    /// Returns the number of deleted sessions.
    pub fn cleanup(
        &self,
        max_age_days: u32,
        max_storage_mb: u64,
    ) -> Result<u32, SessionStoreError> {
        let mut deleted = 0u32;
        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);

        // Phase 1: delete sessions older than max_age_days
        let mut sessions = self.list()?;
        let mut to_delete = Vec::new();
        for meta in &sessions {
            if meta.last_active < cutoff {
                to_delete.push(meta.id.clone());
            }
        }
        for id in &to_delete {
            self.delete(id)?;
            deleted += 1;
        }

        // Remove deleted from the working list
        sessions.retain(|m| !to_delete.contains(&m.id));

        // Phase 2: enforce storage limit
        let max_bytes = max_storage_mb * 1024 * 1024;
        let mut total_bytes = self.total_storage_bytes()?;
        if total_bytes > max_bytes {
            // Sort by last_active ascending (oldest first)
            sessions.sort_by_key(|m| m.last_active);
            for meta in &sessions {
                if total_bytes <= max_bytes {
                    break;
                }
                let dir = self.session_dir(&meta.id);
                let dir_size = dir_size_bytes(&dir);
                self.delete(&meta.id)?;
                deleted += 1;
                total_bytes = total_bytes.saturating_sub(dir_size);
            }
        }

        Ok(deleted)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Atomically write meta.json via a temp file + rename.
    fn write_meta_atomic(
        &self,
        session_id: &str,
        meta: &SessionMeta,
    ) -> Result<(), SessionStoreError> {
        let final_path = self.meta_path(session_id);
        let tmp_path = self.session_dir(session_id).join("meta.json.tmp");
        let content = serde_json::to_string_pretty(meta)?;
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }

    /// Read and parse events.jsonl, skipping malformed lines.
    fn read_events_raw(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionEvent>, SessionStoreError> {
        let path = self.events_path(session_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for (lineno, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionEvent>(&line) {
                Ok(evt) => events.push(evt),
                Err(e) => {
                    warn!(
                        session_id = session_id,
                        line = lineno + 1,
                        error = %e,
                        "skipping malformed event line"
                    );
                }
            }
        }
        Ok(events)
    }

    /// Sum of all file sizes under base_dir.
    fn total_storage_bytes(&self) -> Result<u64, SessionStoreError> {
        Ok(dir_size_bytes(&self.base_dir))
    }
}

/// Recursively compute the total size of files in a directory.
fn dir_size_bytes(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size_bytes(&p);
            } else if let Ok(md) = p.metadata() {
                total += md.len();
            }
        }
    }
    total
}
