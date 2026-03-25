//! ACP (Agent Communication Protocol) client module.
//!
//! Provides subprocess-based communication with AI agent binaries using
//! JSON-RPC over stdin/stdout, agent lifecycle management, and session routing.

pub mod acp_client;
pub mod manager;
pub mod types;
