//! In-memory notification store — keeps the last N notifications for REST queries.

use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Maximum number of notifications to keep.
const MAX_NOTIFICATIONS: usize = 100;

/// A stored notification record.
#[derive(Debug, Clone, Serialize)]
pub struct StoredNotification {
    pub id: u64,
    pub session_id: String,
    pub trigger_type: String,
    pub summary: String,
    pub urgent: bool,
    pub timestamp: DateTime<Utc>,
}

/// Thread-safe ring buffer of recent notifications.
pub struct NotificationStore {
    inner: Mutex<StoreInner>,
}

struct StoreInner {
    entries: VecDeque<StoredNotification>,
    next_id: u64,
}

impl NotificationStore {
    /// Create a new empty notification store.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(StoreInner {
                entries: VecDeque::with_capacity(MAX_NOTIFICATIONS),
                next_id: 1,
            }),
        }
    }

    /// Push a new notification into the store.
    pub fn push(
        &self,
        session_id: String,
        trigger_type: String,
        summary: String,
        urgent: bool,
    ) -> StoredNotification {
        let mut inner = self.inner.lock().unwrap();

        let entry = StoredNotification {
            id: inner.next_id,
            session_id,
            trigger_type,
            summary,
            urgent,
            timestamp: Utc::now(),
        };
        inner.next_id += 1;

        if inner.entries.len() >= MAX_NOTIFICATIONS {
            inner.entries.pop_front();
        }
        inner.entries.push_back(entry.clone());

        entry
    }

    /// Return all stored notifications (newest last).
    pub fn list(&self) -> Vec<StoredNotification> {
        let inner = self.inner.lock().unwrap();
        inner.entries.iter().cloned().collect()
    }

    /// Return notifications with id > `since_id`.
    pub fn list_since(&self, since_id: u64) -> Vec<StoredNotification> {
        let inner = self.inner.lock().unwrap();
        inner
            .entries
            .iter()
            .filter(|n| n.id > since_id)
            .cloned()
            .collect()
    }

    /// Return the number of stored notifications.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().entries.len()
    }

    /// Return true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().entries.is_empty()
    }
}

impl Default for NotificationStore {
    fn default() -> Self {
        Self::new()
    }
}
