//! IM and Tunnel method definitions for JSON-RPC plugin communication.
//!
//! Defines the method names, parameter types, and result types for both
//! IM (Instant Messaging) and Tunnel plugin protocols.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// IM Plugin Methods
// ---------------------------------------------------------------------------

/// Method names for IM plugin JSON-RPC calls.
pub mod im_methods {
    /// Host -> Plugin: Initialize the IM connection.
    pub const INITIALIZE: &str = "im/initialize";
    /// Host -> Plugin: Send a text message to the IM platform.
    pub const SEND_MESSAGE: &str = "im/send_message";
    /// Host -> Plugin: Send an image to the IM platform.
    pub const SEND_IMAGE: &str = "im/send_image";
    /// Host -> Plugin: Graceful shutdown.
    pub const SHUTDOWN: &str = "im/shutdown";
    /// Plugin -> Host: Incoming message from the IM platform.
    pub const ON_MESSAGE: &str = "im/on_message";
    /// Plugin -> Host: IM connection status changed.
    pub const ON_STATUS: &str = "im/on_status";
}

/// Parameters for `im/initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImInitializeParams {
    /// Plugin configuration from plugin.toml [config] section.
    #[serde(default)]
    pub config: serde_json::Value,
    /// Protocol version the host supports.
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
}

/// Result of `im/initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImInitializeResult {
    /// Plugin name as reported by the plugin itself.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Capabilities the plugin supports.
    #[serde(default)]
    pub capabilities: ImCapabilities,
}

/// IM plugin capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImCapabilities {
    /// Whether the plugin supports image sending.
    #[serde(default)]
    pub supports_images: bool,
    /// Whether the plugin supports rich text / markdown.
    #[serde(default)]
    pub supports_markdown: bool,
    /// Maximum message length (0 = unlimited).
    #[serde(default)]
    pub max_message_length: usize,
}

/// Parameters for `im/send_message`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImSendMessageParams {
    /// The text content to send.
    pub text: String,
    /// Optional channel/room/conversation ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Whether this is an urgent/priority message.
    #[serde(default)]
    pub urgent: bool,
}

/// Parameters for `im/send_image`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImSendImageParams {
    /// Base64-encoded image data.
    pub data: String,
    /// MIME type (e.g., "image/png").
    pub mime_type: String,
    /// Optional caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Optional channel/room/conversation ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

/// Notification params for `im/on_message` (plugin -> host).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImOnMessageParams {
    /// The text content of the incoming message.
    pub text: String,
    /// Sender identifier.
    pub sender: String,
    /// Optional channel/room/conversation ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Timestamp (unix millis).
    #[serde(default)]
    pub timestamp: u64,
}

/// Notification params for `im/on_status` (plugin -> host).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImOnStatusParams {
    /// Current connection status.
    pub status: ImConnectionStatus,
    /// Human-readable message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// IM connection status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    Error,
}

// ---------------------------------------------------------------------------
// Tunnel Plugin Methods
// ---------------------------------------------------------------------------

/// Method names for Tunnel plugin JSON-RPC calls.
pub mod tunnel_methods {
    /// Host -> Plugin: Initialize the tunnel.
    pub const INITIALIZE: &str = "tunnel/initialize";
    /// Host -> Plugin: Start the tunnel.
    pub const START: &str = "tunnel/start";
    /// Host -> Plugin: Stop the tunnel.
    pub const STOP: &str = "tunnel/stop";
    /// Host -> Plugin: Health check.
    pub const HEALTH: &str = "tunnel/health";
    /// Plugin -> Host: Tunnel status changed.
    pub const ON_STATUS: &str = "tunnel/on_status";
    /// Plugin -> Host: Tunnel metrics update.
    pub const ON_METRICS: &str = "tunnel/on_metrics";
}

/// Parameters for `tunnel/initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInitializeParams {
    /// Plugin configuration from plugin.toml [config] section.
    #[serde(default)]
    pub config: serde_json::Value,
    /// Local port to tunnel.
    pub local_port: u16,
    /// Desired subdomain or domain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// Result of `tunnel/initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInitializeResult {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Tunnel capabilities.
    #[serde(default)]
    pub capabilities: TunnelCapabilities,
}

/// Tunnel plugin capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TunnelCapabilities {
    /// Whether the tunnel supports custom domains.
    #[serde(default)]
    pub supports_custom_domain: bool,
    /// Whether the tunnel supports TLS.
    #[serde(default)]
    pub supports_tls: bool,
}

/// Result of `tunnel/start`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStartResult {
    /// The public URL of the tunnel.
    pub url: String,
    /// Expiration time (unix secs), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

/// Result of `tunnel/health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelHealthResult {
    /// Whether the tunnel is healthy.
    pub healthy: bool,
    /// Current tunnel URL (if active).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Uptime in seconds.
    #[serde(default)]
    pub uptime_secs: u64,
}

/// Notification params for `tunnel/on_status` (plugin -> host).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelOnStatusParams {
    /// Current tunnel status.
    pub status: TunnelStatus,
    /// Public URL (when status is Ready).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Human-readable reason (for Down/Error states).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Tunnel status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunnelStatus {
    Starting,
    Ready,
    Down,
    Error,
}

/// Notification params for `tunnel/on_metrics` (plugin -> host).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelOnMetricsParams {
    /// Bytes transferred (upload).
    pub bytes_up: u64,
    /// Bytes transferred (download).
    pub bytes_down: u64,
    /// Active connections count.
    pub active_connections: u32,
    /// Requests per minute.
    #[serde(default)]
    pub requests_per_minute: f64,
}

// ---------------------------------------------------------------------------
// Plugin Manifest (plugin.toml)
// ---------------------------------------------------------------------------

/// Parsed plugin.toml manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Plugin metadata from the [plugin] section of plugin.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    /// Plugin identifier (unique).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Semantic version.
    pub version: String,
    /// Plugin type: "im" or "tunnel".
    #[serde(rename = "type")]
    pub plugin_type: PluginType,
    /// Relative path to the executable (within the plugin directory).
    pub executable: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Plugin type discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Im,
    Tunnel,
}

// ---------------------------------------------------------------------------
// Plugin lifecycle states
// ---------------------------------------------------------------------------

/// Plugin lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Discovered but not yet started.
    Pending,
    /// Starting up (initialize handshake in progress).
    Starting,
    /// Ready and operational.
    Ready,
    /// Restarting after a crash (with current attempt count).
    Restarting { attempt: u32 },
    /// Disabled after too many restart failures.
    Disabled { reason: String },
    /// Gracefully stopped.
    Stopped,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginState::Pending => write!(f, "pending"),
            PluginState::Starting => write!(f, "starting"),
            PluginState::Ready => write!(f, "ready"),
            PluginState::Restarting { attempt } => write!(f, "restarting (attempt {attempt})"),
            PluginState::Disabled { reason } => write!(f, "disabled: {reason}"),
            PluginState::Stopped => write!(f, "stopped"),
        }
    }
}

fn default_protocol_version() -> String {
    "1.0".to_string()
}
