// rtb-server: HTTP and WebSocket server for RTB 2.0
//
// Implements the Axum-based web server with REST API endpoints,
// WebSocket transport for real-time terminal I/O, static file
// serving, authentication, and rate limiting.

pub mod api;
pub mod auth;
pub mod router;
pub mod security;
pub mod state;
pub mod static_files;
pub mod ws;

use std::sync::Arc;

use rtb_core::CoreState;
use tokio::sync::RwLock;

use crate::router::create_router;
use crate::state::AppState;

/// Start the RTB HTTP/WebSocket server.
///
/// This binds to `host:port`, wires the Axum router with auth and security
/// middleware, and serves until the process is terminated.
pub async fn start_server(
    core: Arc<CoreState>,
    token: String,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let state = AppState {
        core,
        token: Arc::new(RwLock::new(token)),
    };

    let app = create_router(state);

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("RTB server listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
