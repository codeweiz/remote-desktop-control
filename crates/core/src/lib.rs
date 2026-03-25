// rtb-core: Core library for RTB 2.0
//
// Provides shared types, configuration management, PTY handling,
// session management, and foundational abstractions used by all
// other crates in the workspace.

pub mod config;
pub mod event_bus;
pub mod events;
pub mod pty;
pub mod session;
