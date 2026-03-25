use std::sync::Arc;

use rtb_core::config::Config;
use rtb_core::CoreState;

use crate::daemon;
use crate::Cli;

/// Start the RTB daemon.
///
/// Loads configuration, applies CLI overrides, initialises the core state,
/// generates (or loads) the authentication token, writes a PID file and
/// finally starts the Axum HTTP/WebSocket server.
pub async fn start(cli: &Cli) -> anyhow::Result<()> {
    // 1. Load config (fall back to defaults when the file is missing)
    let mut config = Config::load().unwrap_or_default();

    // 2. Apply CLI overrides – top-level flags as well as Start sub-command
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

    // 3. Initialize core
    let core = Arc::new(CoreState::new(config.clone())?);

    // 4. Generate or load auth token
    let token = daemon::load_or_create_token(&config)?;

    // 5. Print access info
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

    // 6. Write PID file
    daemon::write_pid_file()?;

    // 7. Register Ctrl-C handler to clean up the PID file on exit
    ctrlc::set_handler(move || {
        let _ = daemon::remove_pid_file();
        std::process::exit(0);
    })?;

    // 8. Start server (blocks until shutdown)
    rtb_server::start_server(core, token, &config.server.host, config.server.port).await?;

    // 9. Cleanup on normal exit
    daemon::remove_pid_file()?;

    Ok(())
}
