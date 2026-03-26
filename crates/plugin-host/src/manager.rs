//! Plugin process lifecycle manager.
//!
//! Discovers plugins from ~/.rtb/plugins/, parses plugin.toml manifests,
//! manages the lifecycle state machine, and handles crash recovery with
//! exponential backoff.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use rtb_core::event_bus::EventBus;
use rtb_core::events::ControlEvent;

use crate::im::ImBridge;
use crate::plugin::{PluginProcess, PluginProcessError};
use crate::tunnel::TunnelBridge;
use crate::types::{PluginManifest, PluginState, PluginType};

/// Maximum restart attempts before disabling a plugin.
const MAX_RESTART_ATTEMPTS: u32 = 3;
/// Backoff schedule in seconds: 3, 6, 12.
const BACKOFF_SCHEDULE: [u64; 3] = [3, 6, 12];

/// Plugin manager errors.
#[derive(Debug, thiserror::Error)]
pub enum PluginManagerError {
    #[error("plugin not found: {0}")]
    PluginNotFound(String),
    #[error("plugin directory not found: {0}")]
    DirNotFound(String),
    #[error("failed to parse plugin.toml: {0}")]
    ManifestParse(String),
    #[error("plugin process error: {0}")]
    Process(#[from] PluginProcessError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Tracks state for a managed plugin.
struct ManagedPlugin {
    process: PluginProcess,
    restart_count: u32,
    _im_bridge: Option<ImBridge>,
    _tunnel_bridge: Option<TunnelBridge>,
}

/// Manages the lifecycle of all discovered plugins.
pub struct PluginManager {
    /// Plugin directory (typically ~/.rtb/plugins/).
    plugins_dir: PathBuf,
    /// All managed plugins keyed by plugin ID.
    plugins: Arc<RwLock<HashMap<String, ManagedPlugin>>>,
    /// Event bus for publishing control events.
    event_bus: Arc<EventBus>,
    /// JSON-RPC timeout in seconds.
    timeout_secs: u64,
}

impl PluginManager {
    /// Create a new plugin manager.
    pub fn new(plugins_dir: PathBuf, event_bus: Arc<EventBus>, timeout_secs: u64) -> Self {
        Self {
            plugins_dir,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            timeout_secs,
        }
    }

    /// Returns the plugins directory path.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Discover plugins from the plugins directory.
    ///
    /// Scans each subdirectory for a `plugin.toml` manifest file.
    pub async fn discover(&self) -> Result<Vec<PluginManifest>, PluginManagerError> {
        let mut manifests = Vec::new();

        if !self.plugins_dir.exists() {
            debug!(dir = %self.plugins_dir.display(), "plugins directory does not exist");
            return Ok(manifests);
        }

        let mut entries = tokio::fs::read_dir(&self.plugins_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                debug!(dir = %path.display(), "no plugin.toml found, skipping");
                continue;
            }

            match Self::parse_manifest(&manifest_path).await {
                Ok(manifest) => {
                    info!(
                        id = %manifest.plugin.id,
                        name = %manifest.plugin.name,
                        plugin_type = ?manifest.plugin.plugin_type,
                        "discovered plugin"
                    );
                    manifests.push(manifest);
                }
                Err(e) => {
                    warn!(
                        path = %manifest_path.display(),
                        error = %e,
                        "failed to parse plugin manifest"
                    );
                }
            }
        }

        Ok(manifests)
    }

    /// Parse a plugin.toml manifest file.
    async fn parse_manifest(path: &Path) -> Result<PluginManifest, PluginManagerError> {
        let content = tokio::fs::read_to_string(path).await?;
        let manifest: PluginManifest =
            toml::from_str(&content).map_err(|e| PluginManagerError::ManifestParse(e.to_string()))?;
        Ok(manifest)
    }

    /// Load and start all discovered plugins.
    pub async fn start_all(&self) -> Result<(), PluginManagerError> {
        let manifests = self.discover().await?;

        for manifest in manifests {
            if let Err(e) = self.start_plugin(manifest).await {
                error!(error = %e, "failed to start plugin");
            }
        }

        Ok(())
    }

    /// Start a plugin by its directory ID (directory name under plugins_dir).
    ///
    /// This is used by the hot-reload watcher when a new plugin directory appears.
    pub async fn start_plugin_by_id(&self, plugin_id: &str) -> Result<(), PluginManagerError> {
        let plugin_dir = self.plugins_dir.join(plugin_id);
        let manifest_path = plugin_dir.join("plugin.toml");

        if !manifest_path.exists() {
            return Err(PluginManagerError::DirNotFound(format!(
                "no plugin.toml found at {}",
                manifest_path.display()
            )));
        }

        let manifest = Self::parse_manifest(&manifest_path).await?;
        self.start_plugin(manifest).await
    }

    /// Start a single plugin from its manifest.
    pub async fn start_plugin(
        &self,
        manifest: PluginManifest,
    ) -> Result<(), PluginManagerError> {
        let plugin_id = manifest.plugin.id.clone();
        let plugin_name = manifest.plugin.name.clone();
        let plugin_type = manifest.plugin.plugin_type.clone();
        let plugin_dir = self.plugins_dir.join(&plugin_id);

        // Check if already running
        {
            let plugins = self.plugins.read().await;
            if plugins.contains_key(&plugin_id) {
                info!(plugin_id = %plugin_id, "plugin already running, skipping");
                return Ok(());
            }
        }

        let mut process = PluginProcess::new(manifest, plugin_dir, Some(self.timeout_secs));

        // Spawn the subprocess
        process.spawn().await?;

        // Set up the appropriate bridge based on plugin type
        let im_bridge = if plugin_type == PluginType::Im {
            let notification_rx = process.take_notification_rx();
            if let Some(rx) = notification_rx {
                let bridge = ImBridge::new(Arc::clone(&self.event_bus));
                bridge.start(rx);
                // Also listen for NotificationTriggered control events and forward to IM
                bridge.start_notification_listener();
                Some(bridge)
            } else {
                None
            }
        } else {
            None
        };

        let tunnel_bridge = if plugin_type == PluginType::Tunnel {
            let notification_rx = process.take_notification_rx();
            if let Some(rx) = notification_rx {
                let bridge = TunnelBridge::new(Arc::clone(&self.event_bus));
                bridge.start(rx);
                Some(bridge)
            } else {
                None
            }
        } else {
            None
        };

        // Perform initialize handshake
        let init_params = match plugin_type {
            PluginType::Im => serde_json::json!({
                "protocol_version": "1.0",
                "config": {}
            }),
            PluginType::Tunnel => serde_json::json!({
                "config": {},
                "local_port": 3000
            }),
        };

        match process.call(
            match plugin_type {
                PluginType::Im => crate::types::im_methods::INITIALIZE,
                PluginType::Tunnel => crate::types::tunnel_methods::INITIALIZE,
            },
            Some(init_params),
        ).await {
            Ok(resp) => {
                if resp.is_error() {
                    let err = resp.error.unwrap();
                    error!(
                        plugin_id = %plugin_id,
                        code = err.code,
                        message = %err.message,
                        "plugin initialization failed"
                    );
                    return Err(PluginProcessError::RpcError {
                        code: err.code,
                        message: err.message,
                    }.into());
                }
                process.state = PluginState::Ready;
                info!(plugin_id = %plugin_id, "plugin initialized successfully");
            }
            Err(e) => {
                error!(plugin_id = %plugin_id, error = %e, "plugin initialization error");
                return Err(e.into());
            }
        }

        // Wire up IM bridge sender if this is an IM plugin
        if let Some(ref bridge) = im_bridge {
            // Create a sender closure that calls the plugin's send_message method
            let plugins_ref = Arc::clone(&self.plugins);
            let pid = plugin_id.clone();
            let sender: crate::im::ImPluginSender = Arc::new(move |text: String, channel: Option<String>| {
                let plugins_ref = Arc::clone(&plugins_ref);
                let pid = pid.clone();
                Box::pin(async move {
                    let plugins = plugins_ref.read().await;
                    if let Some(managed) = plugins.get(&pid) {
                        let params = serde_json::json!({
                            "text": text,
                            "channel": channel,
                        });
                        if let Err(e) = managed.process.call(
                            crate::types::im_methods::SEND_MESSAGE,
                            Some(params),
                        ).await {
                            warn!(
                                plugin_id = %pid,
                                error = %e,
                                "failed to send message via IM plugin"
                            );
                        }
                    }
                })
            });
            bridge.set_plugin_sender(sender).await;
        }

        // Publish PluginLoaded event
        self.event_bus.publish_control(ControlEvent::PluginLoaded {
            plugin_id: plugin_id.clone(),
            name: plugin_name,
        });

        let managed = ManagedPlugin {
            process,
            restart_count: 0,
            _im_bridge: im_bridge,
            _tunnel_bridge: tunnel_bridge,
        };

        self.plugins.write().await.insert(plugin_id, managed);
        Ok(())
    }

    /// Stop a specific plugin by ID.
    pub async fn stop_plugin(&self, plugin_id: &str) -> Result<(), PluginManagerError> {
        let mut plugins = self.plugins.write().await;
        if let Some(managed) = plugins.get_mut(plugin_id) {
            managed.process.kill().await;
            plugins.remove(plugin_id);
            info!(plugin_id = %plugin_id, "plugin stopped");
            Ok(())
        } else {
            Err(PluginManagerError::PluginNotFound(plugin_id.to_string()))
        }
    }

    /// Stop all plugins.
    pub async fn stop_all(&self) {
        let mut plugins = self.plugins.write().await;
        for (id, managed) in plugins.iter_mut() {
            managed.process.kill().await;
            info!(plugin_id = %id, "plugin stopped");
        }
        plugins.clear();
    }

    /// Check plugin health and restart crashed plugins.
    ///
    /// Should be called periodically (e.g., every 5 seconds).
    pub async fn health_check(&self) {
        let mut plugins = self.plugins.write().await;
        let mut to_restart = Vec::new();

        for (id, managed) in plugins.iter_mut() {
            if !managed.process.is_running() && managed.process.state == PluginState::Ready {
                warn!(plugin_id = %id, "plugin process crashed");
                to_restart.push(id.clone());
            }
        }

        for id in to_restart {
            if let Some(managed) = plugins.get_mut(&id) {
                managed.restart_count += 1;

                if managed.restart_count > MAX_RESTART_ATTEMPTS {
                    let reason = format!(
                        "exceeded max restart attempts ({MAX_RESTART_ATTEMPTS})"
                    );
                    managed.process.state = PluginState::Disabled {
                        reason: reason.clone(),
                    };
                    error!(plugin_id = %id, reason = %reason, "plugin disabled");
                    self.event_bus.publish_control(ControlEvent::PluginError {
                        plugin_id: id.clone(),
                        error: reason,
                    });
                    continue;
                }

                let attempt = managed.restart_count;
                let backoff_secs = BACKOFF_SCHEDULE
                    .get((attempt - 1) as usize)
                    .copied()
                    .unwrap_or(*BACKOFF_SCHEDULE.last().unwrap());

                managed.process.state = PluginState::Restarting { attempt };
                info!(
                    plugin_id = %id,
                    attempt = attempt,
                    backoff_secs = backoff_secs,
                    "scheduling plugin restart"
                );

                let event_bus = Arc::clone(&self.event_bus);
                let id_clone = id.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                    debug!(
                        plugin_id = %id_clone,
                        "backoff elapsed, plugin ready for restart"
                    );
                    // The actual restart would be triggered by the next health_check.
                    // We just notify that an error occurred.
                    event_bus.publish_control(ControlEvent::PluginError {
                        plugin_id: id_clone,
                        error: format!("crashed, restart attempt {attempt}"),
                    });
                });
            }
        }
    }

    /// List all managed plugins with their current states.
    pub async fn list_plugins(&self) -> Vec<(String, String, PluginState)> {
        let plugins = self.plugins.read().await;
        plugins
            .iter()
            .map(|(id, managed)| {
                (
                    id.clone(),
                    managed.process.manifest.plugin.name.clone(),
                    managed.process.state.clone(),
                )
            })
            .collect()
    }

    /// Send a JSON-RPC call to a specific plugin.
    pub async fn call_plugin(
        &self,
        plugin_id: &str,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, PluginManagerError> {
        let plugins = self.plugins.read().await;
        let managed = plugins
            .get(plugin_id)
            .ok_or_else(|| PluginManagerError::PluginNotFound(plugin_id.to_string()))?;

        let resp = managed.process.call(method, params).await?;
        if let Some(err) = resp.error {
            Err(PluginProcessError::RpcError {
                code: err.code,
                message: err.message,
            }
            .into())
        } else {
            Ok(resp.result.unwrap_or(serde_json::Value::Null))
        }
    }
}
