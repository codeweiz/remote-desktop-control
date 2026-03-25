//! IM Bridge — routes incoming IM messages to EventBus and throttles outgoing.
//!
//! Handles `im/on_message` and `im/on_status` notifications from the IM plugin,
//! and provides outgoing message batching with configurable throttle interval.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, warn};

use rtb_core::event_bus::EventBus;
use rtb_core::events::ControlEvent;

use crate::protocol::JsonRpcNotification;
use crate::types::{im_methods, ImOnMessageParams, ImOnStatusParams, ImConnectionStatus};

/// Default throttle interval for batching outgoing messages (5 seconds).
const DEFAULT_THROTTLE_MS: u64 = 5000;

/// Strips ANSI escape sequences from text.
fn strip_ansi(input: &str) -> String {
    // Match ANSI escape sequences: ESC[ ... final_byte
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check for CSI sequence (ESC [)
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we hit a letter (final byte of CSI: 0x40-0x7E)
                loop {
                    match chars.next() {
                        Some(fc) if fc.is_ascii() && (0x40..=0x7E).contains(&(fc as u8)) => break,
                        None => break,
                        _ => continue,
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence (ESC ])
                chars.next();
                // Skip until BEL (0x07) or ST (ESC \)
                loop {
                    match chars.next() {
                        Some('\x07') => break,
                        Some('\x1b') => {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                                break;
                            }
                        }
                        None => break,
                        _ => continue,
                    }
                }
            }
            // Skip other single-char escape sequences
        } else if c == '\x07' || c == '\x0f' || c == '\x0e' {
            // Skip BEL, SI, SO
            continue;
        } else {
            result.push(c);
        }
    }

    result
}

/// Bridge between IM plugin notifications and the EventBus.
pub struct ImBridge {
    event_bus: Arc<EventBus>,
    /// Outgoing message queue for throttled sending.
    outgoing_tx: mpsc::Sender<String>,
    /// Throttle interval in milliseconds.
    _throttle_ms: u64,
}

impl ImBridge {
    /// Create a new IM bridge.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self::with_throttle(event_bus, DEFAULT_THROTTLE_MS)
    }

    /// Create a new IM bridge with a custom throttle interval.
    pub fn with_throttle(event_bus: Arc<EventBus>, throttle_ms: u64) -> Self {
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<String>(256);

        // Start the throttled outgoing sender
        Self::start_throttle_task(outgoing_rx, throttle_ms);

        Self {
            event_bus,
            outgoing_tx,
            _throttle_ms: throttle_ms,
        }
    }

    /// Start processing incoming notifications from the IM plugin.
    pub fn start(&self, mut notification_rx: mpsc::Receiver<JsonRpcNotification>) {
        let event_bus = Arc::clone(&self.event_bus);

        tokio::spawn(async move {
            while let Some(notif) = notification_rx.recv().await {
                match notif.method.as_str() {
                    im_methods::ON_MESSAGE => {
                        if let Some(params) = notif.params {
                            match serde_json::from_value::<ImOnMessageParams>(params) {
                                Ok(msg) => {
                                    let clean_text = strip_ansi(&msg.text);
                                    debug!(
                                        sender = %msg.sender,
                                        text = %clean_text,
                                        "received IM message"
                                    );
                                    // Route to event bus as a control event.
                                    // In the future, this could map to a specific session
                                    // or command handler.
                                    // For now, we log it. Real routing would depend
                                    // on the IM command parser.
                                    let _ = clean_text; // used in debug above
                                }
                                Err(e) => {
                                    warn!(error = %e, "failed to parse im/on_message params");
                                }
                            }
                        }
                    }
                    im_methods::ON_STATUS => {
                        if let Some(params) = notif.params {
                            match serde_json::from_value::<ImOnStatusParams>(params) {
                                Ok(status) => {
                                    debug!(status = ?status.status, "IM status changed");
                                    match status.status {
                                        ImConnectionStatus::Error
                                        | ImConnectionStatus::Disconnected => {
                                            event_bus.publish_control(
                                                ControlEvent::PluginError {
                                                    plugin_id: "im".to_string(),
                                                    error: status
                                                        .message
                                                        .unwrap_or_else(|| {
                                                            format!("{:?}", status.status)
                                                        }),
                                                },
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "failed to parse im/on_status params");
                                }
                            }
                        }
                    }
                    other => {
                        debug!(method = %other, "unknown IM notification method");
                    }
                }
            }
        });
    }

    /// Queue a message for throttled outgoing delivery.
    pub async fn queue_outgoing(&self, text: String) {
        let clean = strip_ansi(&text);
        if self.outgoing_tx.send(clean).await.is_err() {
            warn!("outgoing message channel closed");
        }
    }

    /// Background task that batches outgoing messages.
    fn start_throttle_task(mut rx: mpsc::Receiver<String>, throttle_ms: u64) {
        tokio::spawn(async move {
            let interval = tokio::time::Duration::from_millis(throttle_ms);
            let mut batch = Vec::new();

            loop {
                // Collect messages for the throttle interval
                let deadline = tokio::time::sleep(interval);
                tokio::pin!(deadline);

                loop {
                    tokio::select! {
                        msg = rx.recv() => {
                            match msg {
                                Some(text) => batch.push(text),
                                None => return, // channel closed
                            }
                        }
                        _ = &mut deadline => {
                            break;
                        }
                    }
                }

                if !batch.is_empty() {
                    let combined = batch.join("\n---\n");
                    debug!(
                        messages = batch.len(),
                        total_len = combined.len(),
                        "flushing batched outgoing messages"
                    );
                    // In a real implementation, this would call the plugin's send_message.
                    // For now, we just log. The PluginManager would provide the send handle.
                    batch.clear();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_basic() {
        assert_eq!(strip_ansi("hello"), "hello");
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[1;32mbold green\x1b[0m"), "bold green");
    }

    #[test]
    fn test_strip_ansi_complex() {
        // Cursor movement
        assert_eq!(strip_ansi("\x1b[2Jcleared"), "cleared");
        // Mixed content
        assert_eq!(
            strip_ansi("before\x1b[31mred\x1b[0mafter"),
            "beforeredafter"
        );
    }

    #[test]
    fn test_strip_ansi_empty() {
        assert_eq!(strip_ansi(""), "");
        assert_eq!(strip_ansi("\x1b[m"), "");
    }
}
