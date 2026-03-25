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

fn main() -> anyhow::Result<()> {
    // Initialize tracing (respects RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match &cli.command {
        // Explicit subcommands
        Some(Commands::Stop) => daemon::stop_daemon()?,
        Some(Commands::Status) => daemon::print_status()?,
        Some(Commands::Session { action }) => commands::session::handle(action)?,

        // `rtb start ...` or just `rtb` (no subcommand) both start the daemon
        Some(Commands::Start { .. }) | None => {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(commands::start::start(&cli))?;
        }
    }

    Ok(())
}
