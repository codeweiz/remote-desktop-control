// rtb-plugin-host: Plugin host runtime for RTB 2.0
//
// Manages the lifecycle of plugins, providing a sandboxed
// execution environment, plugin discovery, loading, and
// inter-plugin communication channels.

pub mod im;
pub mod manager;
pub mod plugin;
pub mod protocol;
pub mod tunnel;
pub mod types;
pub mod watcher;
