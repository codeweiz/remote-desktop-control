use bytes::Bytes;
use std::collections::VecDeque;
use std::sync::Mutex;

/// Thread-safe ring buffer for storing PTY output history.
///
/// Each entry is a `(seq, data)` tuple where `seq` is a monotonically
/// increasing sequence number assigned by the PTY session. When the
/// buffer reaches capacity, the oldest entries are evicted.
pub struct RingBuffer {
    buffer: Mutex<VecDeque<(u64, Bytes)>>,
    capacity: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Push a new entry into the buffer. If the buffer is at capacity,
    /// the oldest entry is evicted first.
    pub fn push(&self, seq: u64, data: Bytes) {
        let mut buf = self.buffer.lock().unwrap();
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back((seq, data));
    }

    /// Return all entries with sequence number strictly greater than `seq`.
    pub fn get_since(&self, seq: u64) -> Vec<(u64, Bytes)> {
        let buf = self.buffer.lock().unwrap();
        buf.iter()
            .filter(|(s, _)| *s > seq)
            .cloned()
            .collect()
    }

    /// Return the last `n` entries (or all entries if fewer than `n` exist).
    pub fn get_last_n(&self, n: usize) -> Vec<(u64, Bytes)> {
        let buf = self.buffer.lock().unwrap();
        let len = buf.len();
        let skip = len.saturating_sub(n);
        buf.iter().skip(skip).cloned().collect()
    }

    /// Return the current number of entries in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }

    /// Return true if the buffer contains no entries.
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().is_empty()
    }

    /// Return the sequence number of the most recent entry, or 0 if empty.
    pub fn last_seq(&self) -> u64 {
        self.buffer
            .lock()
            .unwrap()
            .back()
            .map(|(seq, _)| *seq)
            .unwrap_or(0)
    }
}
