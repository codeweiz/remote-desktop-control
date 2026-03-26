// rtb-server: HTTP and WebSocket server for RTB 2.0
//
// Implements the Axum-based web server with REST API endpoints,
// WebSocket transport for real-time terminal I/O, static file
// serving, authentication, rate limiting, and structured access logging.

pub mod api;
pub mod auth;
pub mod blocklist;
pub mod logging;
pub mod rate_limit;
pub mod router;
pub mod security;
pub mod state;
pub mod static_files;
pub mod ws;

use std::sync::Arc;
use std::time::Duration;

use rtb_core::CoreState;
use tokio::sync::RwLock;

use crate::blocklist::IpBlocklist;
use crate::rate_limit::RateLimiter;
use crate::router::create_router;
use crate::state::AppState;

/// Start the RTB HTTP/WebSocket server.
///
/// This binds to `host:port`, wires the Axum router with auth, security,
/// and access logging middleware, and serves until the process is terminated.
pub async fn start_server(
    core: Arc<CoreState>,
    token: String,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    let blocklist = Arc::new(IpBlocklist::new(Vec::new()));
    let rate_limiter = Arc::new(RateLimiter::new());

    let state = AppState {
        core,
        token: Arc::new(RwLock::new(token)),
        rate_limiter,
        blocklist: blocklist.clone(),
    };

    // Background task: clean up expired bans every 5 minutes.
    tokio::spawn({
        let blocklist = blocklist;
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
            loop {
                interval.tick().await;
                blocklist.cleanup_expired();
            }
        }
    });

    let app = create_router(state);

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("RTB server listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
