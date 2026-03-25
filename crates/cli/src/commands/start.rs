use std::sync::Arc;

use rtb_core::config::Config;
use rtb_core::CoreState;

use crate::daemon;
use crate::Cli;

/// Start the RTB daemon.
///
/// Loads configuration, initializes logging, applies CLI overrides, performs
/// crash recovery, restores sessions, initialises the core state, generates
/// (or loads) the authentication token, writes a PID file, registers signal
/// handlers, and finally starts the Axum HTTP/WebSocket server.
pub async fn start(cli: &Cli) -> anyhow::Result<()> {
    // 1. Load config (fall back to defaults when the file is missing)
    let mut config = Config::load().unwrap_or_default();

    // 2. Initialize structured logging (console + file appender) before anything else
    rtb_server::logging::setup_logging(&config)?;

    // 3. Apply CLI overrides – top-level flags as well as Start sub-command
    //    flags are merged so that `rtb --port 8080` and `rtb start --port 8080`
    //    behave identically.
    let (cmd_port, cmd_host) = match &cli.command {
        Some(crate::Commands::Start { port, host }) => (port.clone(), host.clone()),
        _ => (None, None),
    };

    if let Some(port) = cli.port.or(cmd_port) {
        config.server.port = port;
    }
    if let Some(ref host) = cli.host.clone().or(cmd_host) {
        config.server.host = host.clone();
    }
    if let Some(ref shell) = cli.shell {
        config.server.shell = shell.clone();
    }

    // 4. Crash recovery — detect and clean up stale PID files
    daemon::check_stale_pid()?;

    // 5. Session restore — mark orphaned running/suspended sessions as crashed
    if let Err(e) = daemon::restore_sessions() {
        tracing::warn!(error = %e, "Session restore encountered an error (continuing)");
    }

    // 6. Initialize core
    let core = Arc::new(CoreState::new(config.clone())?);

    // 7. Generate or load auth token
    let token = daemon::load_or_create_token(&config)?;

    // 8. Print access info
    let url = format!(
        "http://{}:{}?token={}",
        config.server.host, config.server.port, token
    );
    println!();
    println!("  RTB 2.0 is running!");
    println!();
    println!("  Local:   {}", url);
    println!("  Token:   {}", token);
    println!();

    // 9. Write PID file
    daemon::write_pid_file()?;

    // 10. Register signal handlers for graceful shutdown
    //     On SIGTERM/SIGINT: log, remove PID file, exit.
    //     We use tokio::signal for async-safe handling inside the runtime.
    let shutdown = async {
        // Wait for either SIGINT (Ctrl-C) or SIGTERM
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");

            tokio::select! {
                _ = ctrl_c => {},
                _ = sigterm.recv() => {},
            }
        }

        #[cfg(not(unix))]
        {
            ctrl_c.await.ok();
        }

        tracing::info!("Shutting down...");
        if let Err(e) = daemon::remove_pid_file() {
            tracing::warn!(error = %e, "Failed to remove PID file during shutdown");
        }
    };

    // 11. Start server with graceful shutdown
    let state = {
        use tokio::sync::RwLock;

        rtb_server::state::AppState {
            core,
            token: Arc::new(RwLock::new(token)),
        }
    };

    let app = rtb_server::router::create_router(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("RTB server listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    // 12. Final cleanup on normal exit (PID file may already be removed by signal handler)
    let _ = daemon::remove_pid_file();

    Ok(())
}
