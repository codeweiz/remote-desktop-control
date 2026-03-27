/// OSC 10/11 color query interceptor.
///
/// TUI apps (Neovim, Helix, bubbletea) query terminal foreground/background
/// colors via OSC 10;? and OSC 11;? escape sequences. Without interception,
/// these queries travel over WebSocket round-trip, adding latency.
///
/// This interceptor detects queries in the PTY output stream and writes
/// responses directly back to the PTY stdin, bypassing the network.
use std::io::Write;
use std::sync::{Arc, Mutex};

use tracing::debug;

/// Maximum bytes kept from the previous chunk for cross-boundary detection.
const PARTIAL_BUF_CAP: usize = 16;

/// Intercepts OSC 10/11 color-query escape sequences from terminal output and
/// responds with the configured palette, writing directly back to the PTY.
pub struct OscColorResponder {
    /// Response bytes for OSC 10 (foreground color query).
    osc10_response: Vec<u8>,
    /// Response bytes for OSC 11 (background color query).
    osc11_response: Vec<u8>,
    /// PTY stdin writer, set after the PTY is opened.
    writer: Option<Arc<Mutex<Box<dyn Write + Send>>>>,
    /// Trailing bytes from the previous chunk, used for cross-boundary matching.
    partial: Vec<u8>,
}

/// The four OSC query patterns we scan for, paired with whether the match is
/// an OSC 10 query (`true`) or OSC 11 query (`false`).
///
/// Each query has two forms (ST terminator `ESC \` or BEL `\x07`).
/// We only match the *query* form (containing `?`), never set-color commands.
const PATTERNS: &[(&[u8], bool)] = &[
    (b"\x1b]10;?\x1b\\", true),  // OSC 10 query, ST terminator
    (b"\x1b]10;?\x07", true),    // OSC 10 query, BEL terminator
    (b"\x1b]11;?\x1b\\", false), // OSC 11 query, ST terminator
    (b"\x1b]11;?\x07", false),   // OSC 11 query, BEL terminator
];

impl OscColorResponder {
    /// Create a responder pre-loaded with a dark theme palette.
    ///
    /// - Foreground (OSC 10): `rgb:c8c8/c8c8/d8d8` — light gray
    /// - Background (OSC 11): `rgb:0d0d/1111/1717` — near-black, matching #0d1117
    pub fn new_dark_theme() -> Self {
        Self {
            osc10_response: b"\x1b]10;rgb:c8c8/c8c8/d8d8\x1b\\".to_vec(),
            osc11_response: b"\x1b]11;rgb:0d0d/1111/1717\x1b\\".to_vec(),
            writer: None,
            partial: Vec::with_capacity(PARTIAL_BUF_CAP),
        }
    }

    /// Attach the PTY writer so the responder can send replies.
    pub fn set_writer(&mut self, writer: Arc<Mutex<Box<dyn Write + Send>>>) {
        self.writer = Some(writer);
    }

    /// Scan a chunk of PTY output for OSC color queries and respond.
    ///
    /// When a query is detected the matching response is written directly to
    /// the PTY stdin via the stored writer, bypassing the network path.
    ///
    /// Returns the data unchanged — we do **not** strip queries from the
    /// stream because that would require reallocating the buffer.
    pub fn intercept<'a>(&mut self, data: &'a [u8]) -> &'a [u8] {
        if self.writer.is_none() || data.is_empty() {
            // Maintain partial buffer even when we can't respond yet.
            self.update_partial(data);
            return data;
        }

        // Build a combined view: partial tail of previous chunk + current chunk.
        // This lets us detect queries that straddle two consecutive reads.
        let combined: Vec<u8> = if self.partial.is_empty() {
            // Fast path — no allocation needed for matching; we still need a
            // Vec for the search helper below, but we avoid copying partial.
            data.to_vec()
        } else {
            let mut v = Vec::with_capacity(self.partial.len() + data.len());
            v.extend_from_slice(&self.partial);
            v.extend_from_slice(data);
            v
        };

        for &(pattern, is_osc10) in PATTERNS {
            // Search for every occurrence of `pattern` in the combined buffer.
            let mut start = 0;
            while let Some(pos) = find_subsequence(&combined[start..], pattern) {
                let response = if is_osc10 {
                    &self.osc10_response
                } else {
                    &self.osc11_response
                };

                // Write the response directly to PTY stdin.
                if let Some(ref writer) = self.writer {
                    if let Ok(mut w) = writer.lock() {
                        let _ = w.write_all(response);
                        debug!(
                            query = if is_osc10 { "OSC 10" } else { "OSC 11" },
                            "responded to color query"
                        );
                    }
                }

                start += pos + pattern.len();
            }
        }

        // Keep the tail of the current chunk for next time.
        self.update_partial(data);

        data
    }

    /// Keep the last `PARTIAL_BUF_CAP` bytes for cross-boundary detection.
    fn update_partial(&mut self, data: &[u8]) {
        self.partial.clear();
        if data.len() <= PARTIAL_BUF_CAP {
            self.partial.extend_from_slice(data);
        } else {
            self.partial
                .extend_from_slice(&data[data.len() - PARTIAL_BUF_CAP..]);
        }
    }
}

/// Find the first occurrence of `needle` in `haystack`, returning offset.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper that creates a responder wired to a shared buffer we can inspect.
    fn setup() -> (OscColorResponder, Arc<Mutex<Vec<u8>>>) {
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = Arc::clone(&buf);

        // Wrap the Vec<u8> in a Cursor so it implements Write.
        let writer: Box<dyn Write + Send> = Box::new(CursorVec(Arc::clone(&buf)));
        let writer_arc = Arc::new(Mutex::new(writer));

        let mut osc = OscColorResponder::new_dark_theme();
        osc.set_writer(writer_arc);

        (osc, buf_clone)
    }

    /// A Write adapter that appends to a shared Vec<u8>.
    struct CursorVec(Arc<Mutex<Vec<u8>>>);

    impl Write for CursorVec {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn responds_to_osc10_st() {
        let (mut osc, buf) = setup();
        let query = b"\x1b]10;?\x1b\\";
        let returned = osc.intercept(query);
        assert_eq!(returned, query);

        let written = buf.lock().unwrap().clone();
        assert_eq!(written, b"\x1b]10;rgb:c8c8/c8c8/d8d8\x1b\\");
    }

    #[test]
    fn responds_to_osc10_bel() {
        let (mut osc, buf) = setup();
        let query = b"\x1b]10;?\x07";
        osc.intercept(query);

        let written = buf.lock().unwrap().clone();
        assert_eq!(written, b"\x1b]10;rgb:c8c8/c8c8/d8d8\x1b\\");
    }

    #[test]
    fn responds_to_osc11_st() {
        let (mut osc, buf) = setup();
        let query = b"\x1b]11;?\x1b\\";
        osc.intercept(query);

        let written = buf.lock().unwrap().clone();
        assert_eq!(written, b"\x1b]11;rgb:0d0d/1111/1717\x1b\\");
    }

    #[test]
    fn responds_to_osc11_bel() {
        let (mut osc, buf) = setup();
        let query = b"\x1b]11;?\x07";
        osc.intercept(query);

        let written = buf.lock().unwrap().clone();
        assert_eq!(written, b"\x1b]11;rgb:0d0d/1111/1717\x1b\\");
    }

    #[test]
    fn ignores_non_query_osc() {
        let (mut osc, buf) = setup();
        // An OSC 10 *set* command (no `?`), should NOT trigger a response.
        let set_cmd = b"\x1b]10;rgb:ffff/0000/0000\x1b\\";
        osc.intercept(set_cmd);

        let written = buf.lock().unwrap().clone();
        assert!(written.is_empty(), "should not respond to set commands");
    }

    #[test]
    fn cross_boundary_detection() {
        let (mut osc, buf) = setup();
        // Split the OSC 11 BEL query across two chunks.
        let part1 = b"\x1b]11;";
        let part2 = b"?\x07";

        osc.intercept(part1);
        assert!(buf.lock().unwrap().is_empty(), "no response from partial");

        osc.intercept(part2);
        let written = buf.lock().unwrap().clone();
        assert_eq!(
            written, b"\x1b]11;rgb:0d0d/1111/1717\x1b\\",
            "should detect query across chunk boundary"
        );
    }

    #[test]
    fn multiple_queries_in_one_chunk() {
        let (mut osc, buf) = setup();
        // Both OSC 10 and OSC 11 queries in the same chunk.
        let mut chunk = Vec::new();
        chunk.extend_from_slice(b"\x1b]10;?\x07");
        chunk.extend_from_slice(b"some data");
        chunk.extend_from_slice(b"\x1b]11;?\x07");

        osc.intercept(&chunk);

        let written = buf.lock().unwrap().clone();
        // Should contain both responses.
        let mut expected = Vec::new();
        expected.extend_from_slice(b"\x1b]10;rgb:c8c8/c8c8/d8d8\x1b\\");
        expected.extend_from_slice(b"\x1b]11;rgb:0d0d/1111/1717\x1b\\");
        assert_eq!(written, expected);
    }

    #[test]
    fn no_response_without_writer() {
        let mut osc = OscColorResponder::new_dark_theme();
        // No writer set — intercept should still return data without panicking.
        let query = b"\x1b]10;?\x07";
        let returned = osc.intercept(query);
        assert_eq!(returned, query);
    }
}
