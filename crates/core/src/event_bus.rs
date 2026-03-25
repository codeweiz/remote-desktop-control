use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, warn};

use crate::events::{ControlEvent, DataEvent, SessionId};

const BROADCAST_CAPACITY: usize = 256;
const DATA_CHANNEL_CAPACITY: usize = 1024;

/// Central event bus implementing a hybrid channel design:
/// - **Broadcast channel** for low-frequency control events (session lifecycle, agent status, tunnel/plugin events)
/// - **Per-session mpsc channels** for high-volume data events (PTY output, agent messages)
pub struct EventBus {
    control_tx: broadcast::Sender<Arc<ControlEvent>>,
    /// Per-session data channels: each session can have multiple subscribers.
    /// Each subscriber gets its own mpsc::Sender stored here; the corresponding
    /// Receiver is returned to the caller of `create_data_subscriber`.
    session_channels: DashMap<SessionId, Vec<mpsc::Sender<DataEvent>>>,
}

impl EventBus {
    /// Create a new EventBus with default channel capacities.
    pub fn new() -> Self {
        let (control_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            control_tx,
            session_channels: DashMap::new(),
        }
    }

    // ---- Control events (broadcast) ----

    /// Publish a control event to all current subscribers.
    pub fn publish_control(&self, event: ControlEvent) {
        let event = Arc::new(event);
        // It is fine if there are no receivers — send returns Err in that case.
        match self.control_tx.send(event) {
            Ok(n) => {
                debug!(receivers = n, "published control event");
            }
            Err(_) => {
                debug!("published control event but no active subscribers");
            }
        }
    }

    /// Subscribe to control events. Returns a broadcast Receiver.
    pub fn subscribe_control(&self) -> broadcast::Receiver<Arc<ControlEvent>> {
        self.control_tx.subscribe()
    }

    // ---- Data events (per-session mpsc) ----

    /// Create a new data event subscriber for the given session.
    /// Returns the receiving end of an mpsc channel (capacity 1024).
    /// Multiple subscribers can be created per session; each receives a
    /// clone of every published data event.
    pub fn create_data_subscriber(&self, session_id: &str) -> mpsc::Receiver<DataEvent> {
        let (tx, rx) = mpsc::channel(DATA_CHANNEL_CAPACITY);
        self.session_channels
            .entry(session_id.to_string())
            .or_default()
            .push(tx);
        rx
    }

    /// Publish a data event to all subscribers of the given session.
    /// Dead senders (whose receivers have been dropped) are cleaned up
    /// automatically during this call.
    pub async fn publish_data(&self, session_id: &str, event: DataEvent) {
        let mut entry = match self.session_channels.get_mut(session_id) {
            Some(entry) => entry,
            None => {
                debug!(session_id, "no subscribers for session, event dropped");
                return;
            }
        };

        let senders = entry.value_mut();
        let mut dead_indices = Vec::new();

        for (i, tx) in senders.iter().enumerate() {
            if tx.send(event.clone()).await.is_err() {
                // Receiver has been dropped — mark for removal.
                dead_indices.push(i);
            }
        }

        // Remove dead senders in reverse order to preserve indices.
        for i in dead_indices.into_iter().rev() {
            let _removed = senders.swap_remove(i);
            warn!(session_id, "removed dead data subscriber");
        }
    }

    /// Remove all data channels for a session, dropping all senders.
    /// This causes all outstanding receivers to return `None` on their next recv.
    pub fn remove_session(&self, session_id: &str) {
        if self.session_channels.remove(session_id).is_some() {
            debug!(session_id, "removed session channels");
        }
    }

    // ---- Introspection helpers (used by tests) ----

    /// Check whether a session has any registered data subscribers.
    pub fn has_session(&self, session_id: &str) -> bool {
        self.session_channels.contains_key(session_id)
    }

    /// Return the number of active data subscribers for a session.
    pub fn subscriber_count(&self, session_id: &str) -> usize {
        self.session_channels
            .get(session_id)
            .map(|entry| entry.value().len())
            .unwrap_or(0)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
