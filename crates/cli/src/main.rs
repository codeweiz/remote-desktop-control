mod commands;
mod daemon;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rtb", about = "Remote Terminal Bridge 2.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Port to listen on
    #[arg(long, default_value = "3000")]
    pub port: Option<u16>,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: Option<String>,

    /// Default shell
    #[arg(long)]
    pub shell: Option<String>,

    /// Don't print QR code
    #[arg(long)]
    pub no_qr: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the RTB daemon (default when no subcommand)
    Start {
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        host: Option<String>,
    },
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Token management
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List all sessions
    List,
    /// Create a new terminal session
    New {
        /// Optional session name
        name: Option<String>,
        /// Command to run instead of default shell
        #[arg(long)]
        cmd: Option<String>,
    },
    /// Kill a session
    Kill {
        /// Session ID to kill
        id: String,
    },
}

#[derive(Subcommand)]
pub enum TokenAction {
    /// Rotate the authentication token
    Rotate,
    /// Show the current token
    Show,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        // Explicit subcommands
        Some(Commands::Stop) => {
            init_basic_tracing();
            daemon::stop_daemon()?;
        }
        Some(Commands::Status) => {
            init_basic_tracing();
            daemon::print_status()?;
        }
        Some(Commands::Session { action }) => {
            init_basic_tracing();
            commands::session::handle(action)?;
        }
        Some(Commands::Token { action }) => {
            init_basic_tracing();
            commands::token::handle(action)?;
        }

        // `rtb start ...` or just `rtb` (no subcommand) both start the daemon.
        // Tracing is initialized inside start() via setup_logging() which
        // reads the config for log level and sets up file appenders.
        Some(Commands::Start { .. }) | None => {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(commands::start::start(&cli))?;
        }
    }

    Ok(())
}

/// Simple tracing init for non-daemon commands (stop, status, session).
fn init_basic_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();
}
