//! Tunnel Bridge — routes tunnel status/metrics notifications to EventBus.
//!
//! Handles `tunnel/on_status` and `tunnel/on_metrics` notifications from
//! the tunnel plugin and emits TunnelReady/TunnelDown control events.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use rtb_core::event_bus::EventBus;
use rtb_core::events::ControlEvent;

use crate::protocol::JsonRpcNotification;
use crate::types::{tunnel_methods, TunnelOnMetricsParams, TunnelOnStatusParams, TunnelStatus};

/// Bridge between Tunnel plugin notifications and the EventBus.
pub struct TunnelBridge {
    event_bus: Arc<EventBus>,
}

impl TunnelBridge {
    /// Create a new tunnel bridge.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }

    /// Start processing incoming notifications from the tunnel plugin.
    pub fn start(&self, mut notification_rx: mpsc::Receiver<JsonRpcNotification>) {
        let event_bus = Arc::clone(&self.event_bus);

        tokio::spawn(async move {
            while let Some(notif) = notification_rx.recv().await {
                match notif.method.as_str() {
                    tunnel_methods::ON_STATUS => {
                        if let Some(params) = notif.params {
                            match serde_json::from_value::<TunnelOnStatusParams>(params) {
                                Ok(status) => {
                                    Self::handle_status_change(&event_bus, status);
                                }
                                Err(e) => {
                                    warn!(
                                        error = %e,
                                        "failed to parse tunnel/on_status params"
                                    );
                                }
                            }
                        }
                    }
                    tunnel_methods::ON_METRICS => {
                        if let Some(params) = notif.params {
                            match serde_json::from_value::<TunnelOnMetricsParams>(params) {
                                Ok(metrics) => {
                                    debug!(
                                        bytes_up = metrics.bytes_up,
                                        bytes_down = metrics.bytes_down,
                                        connections = metrics.active_connections,
                                        rpm = metrics.requests_per_minute,
                                        "tunnel metrics update"
                                    );
                                    // Metrics are logged; in the future they could be
                                    // published to a metrics sink or data channel.
                                }
                                Err(e) => {
                                    warn!(
                                        error = %e,
                                        "failed to parse tunnel/on_metrics params"
                                    );
                                }
                            }
                        }
                    }
                    other => {
                        debug!(method = %other, "unknown tunnel notification method");
                    }
                }
            }
        });
    }

    /// Handle a tunnel status change notification.
    fn handle_status_change(event_bus: &EventBus, status: TunnelOnStatusParams) {
        match status.status {
            TunnelStatus::Ready => {
                let url = status.url.unwrap_or_else(|| "unknown".to_string());
                info!(url = %url, "tunnel is ready");
                event_bus.publish_control(ControlEvent::TunnelReady { url });
            }
            TunnelStatus::Down => {
                let reason = status
                    .reason
                    .unwrap_or_else(|| "unknown reason".to_string());
                warn!(reason = %reason, "tunnel is down");
                event_bus.publish_control(ControlEvent::TunnelDown { reason });
            }
            TunnelStatus::Error => {
                let reason = status.reason.unwrap_or_else(|| "tunnel error".to_string());
                warn!(reason = %reason, "tunnel error");
                event_bus.publish_control(ControlEvent::TunnelDown { reason });
            }
            TunnelStatus::Starting => {
                info!("tunnel is starting");
            }
        }
    }
}
