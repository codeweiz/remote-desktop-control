// OSC color responder — implemented in Task 4
//
// Stub with the method signatures used by PtySession's reader loop.

use std::io::Write;
use std::sync::{Arc, Mutex};

/// Intercepts OSC color-query escape sequences from terminal output and
/// responds with the dark-theme palette. Full implementation in Task 4.
pub struct OscColorResponder {
    _writer: Option<Arc<Mutex<Box<dyn Write + Send>>>>,
}

impl OscColorResponder {
    /// Create a responder pre-loaded with a dark theme palette.
    pub fn new_dark_theme() -> Self {
        Self { _writer: None }
    }

    /// Attach the PTY writer so the responder can send replies.
    pub fn set_writer(&mut self, writer: Arc<Mutex<Box<dyn Write + Send>>>) {
        self._writer = Some(writer);
    }

    /// Scan a chunk of PTY output for OSC color queries and respond.
    /// Returns the data unchanged (no filtering in this stub).
    pub fn intercept<'a>(&mut self, data: &'a [u8]) -> &'a [u8] {
        data
    }
}
