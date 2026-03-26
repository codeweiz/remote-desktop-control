//! ACP (Agent Communication Protocol) client module.
//!
//! Provides subprocess-based communication with AI agent binaries using
//! JSON-RPC over stdin/stdout, agent lifecycle management, and session routing.

pub mod acp_backend;
pub mod claude_bridge;
pub mod claude_sdk;
pub mod error_classify;
pub mod event;
pub mod manager;
pub mod native_acp;
