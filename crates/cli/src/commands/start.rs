use std::path::PathBuf;
use std::sync::Arc;

use qrcode::QrCode;
use rtb_core::config::Config;
use rtb_core::task_pool::scheduler::{SchedulerConfig, TaskScheduler};
use rtb_core::CoreState;
use rtb_plugin_host::manager::PluginManager;

use crate::daemon;
use crate::Cli;

/// Print a QR code to the terminal using Unicode block characters.
/// The QR code encodes the given URL for easy mobile scanning.
fn print_qr_code(url: &str) {
    if let Ok(code) = QrCode::new(url.as_bytes()) {
        let string = code
            .render::<char>()
            .quiet_zone(false)
            .module_dimensions(2, 1)
            .build();
        println!("{}", string);
    }
}

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

    // 7. Load tasks from disk into the task pool
    if let Err(e) = core.task_pool.load().await {
        tracing::warn!(error = %e, "Failed to load task pool from disk (continuing with empty pool)");
    }

    // 8. Start task pool scheduler as a background task
    let scheduler_config = SchedulerConfig {
        max_concurrent: config.task_pool.max_concurrent,
        auto_start: config.task_pool.auto_start,
        poll_interval_secs: 5,
    };
    let task_scheduler = TaskScheduler::new(scheduler_config, Arc::clone(&core.task_pool));
    let scheduler_handle = task_scheduler.start();
    tracing::info!("Task pool scheduler started");

    // 9. Start plugin manager (discovers and spawns plugins from ~/.rtb/plugins/)
    let plugins_dir = PathBuf::from(&config.plugins.dir);
    let plugin_manager = Arc::new(PluginManager::new(
        plugins_dir,
        Arc::clone(&core.event_bus),
        config.plugins.jsonrpc_timeout_secs,
    ));
    if let Err(e) = plugin_manager.start_all().await {
        tracing::warn!(error = %e, "Plugin manager start_all encountered errors (continuing)");
    }

    // Background task: plugin health checks every 10 seconds
    tokio::spawn({
        let pm = Arc::clone(&plugin_manager);
        async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                pm.health_check().await;
            }
        }
    });

    // 10. Generate or load auth token
    let token = daemon::load_or_create_token(&config)?;

    // 11. Print access info
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

    // Print QR code for easy mobile access (unless --no-qr flag is set)
    if !cli.no_qr {
        print_qr_code(&url);
        println!();
    }

    // 12. Write PID file
    daemon::write_pid_file()?;

    // 13. Register signal handlers for graceful shutdown
    //     On SIGTERM/SIGINT: save state, stop subsystems, remove PID file, exit.
    let shutdown_core = Arc::clone(&core);
    let shutdown_plugin_manager = Arc::clone(&plugin_manager);
    let shutdown = async move {
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

        // Stop the task pool scheduler
        scheduler_handle.stop();
        tracing::info!("Task scheduler stopped");

        // Save the task pool to disk
        if let Err(e) = shutdown_core.task_pool.save().await {
            tracing::warn!(error = %e, "Failed to save task pool during shutdown");
        } else {
            tracing::info!("Task pool saved to disk");
        }

        // Stop all plugins
        shutdown_plugin_manager.stop_all().await;
        tracing::info!("All plugins stopped");

        // Mark active sessions with appropriate status:
        // - Terminal (Running) -> Exited
        // - Agent (Running) -> Suspended
        if let Ok(sessions) = shutdown_core.session_store.list() {
            for mut meta in sessions {
                if meta.status == rtb_core::session::types::SessionStatus::Running {
                    let new_status = match meta.session_type {
                        rtb_core::session::types::SessionType::Terminal => {
                            rtb_core::session::types::SessionStatus::Exited
                        }
                        rtb_core::session::types::SessionType::Agent => {
                            rtb_core::session::types::SessionStatus::Suspended
                        }
                    };
                    meta.status = new_status;
                    if let Err(e) = shutdown_core.session_store.update_meta(&meta.id, &meta) {
                        tracing::warn!(session_id = %meta.id, error = %e, "Failed to update session on shutdown");
                    }
                }
            }
            tracing::info!("Active sessions saved");
        }

        if let Err(e) = daemon::remove_pid_file() {
            tracing::warn!(error = %e, "Failed to remove PID file during shutdown");
        }
    };

    // 14. Start server with graceful shutdown
    let blocklist = Arc::new(rtb_server::blocklist::IpBlocklist::new(Vec::new()));
    let rate_limiter = Arc::new(rtb_server::rate_limit::RateLimiter::new());

    // Background task: clean up expired bans every 5 minutes.
    tokio::spawn({
        let blocklist = blocklist.clone();
        async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(5 * 60));
            loop {
                interval.tick().await;
                blocklist.cleanup_expired();
            }
        }
    });

    let state = {
        use tokio::sync::RwLock;

        rtb_server::state::AppState {
            core: Arc::clone(&core),
            token: Arc::new(RwLock::new(token)),
            rate_limiter,
            blocklist,
            plugin_manager: Some(Arc::clone(&plugin_manager)),
        }
    };

    // Background task: listen for NotificationTriggered control events and
    // record them in the notification store so that GET /api/v1/notifications works.
    {
        let notification_store = Arc::clone(&core.notification_store);
        let mut control_rx = core.event_bus.subscribe_control();
        tokio::spawn(async move {
            loop {
                match control_rx.recv().await {
                    Ok(event) => {
                        if let rtb_core::events::ControlEvent::NotificationTriggered {
                            session_id,
                            trigger_type,
                            summary,
                            urgent,
                        } = event.as_ref()
                        {
                            notification_store.push(
                                session_id.clone(),
                                trigger_type.clone(),
                                summary.clone(),
                                *urgent,
                            );
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "notification store listener lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });
    }

    let app = rtb_server::router::create_router(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("RTB server listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    // 15. Final cleanup on normal exit (PID file may already be removed by signal handler)
    let _ = daemon::remove_pid_file();

    Ok(())
}
