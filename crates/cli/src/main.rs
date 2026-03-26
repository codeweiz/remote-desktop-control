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
    /// Agent session management
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Plugin management
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Tunnel management
    Tunnel {
        #[command(subcommand)]
        action: TunnelAction,
    },
    /// Task queue management
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
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

#[derive(Subcommand)]
pub enum AgentAction {
    /// Create a new agent session
    New {
        /// Optional agent session name
        name: Option<String>,
        /// Agent provider (e.g., claude-code)
        #[arg(long, default_value = "claude-code")]
        provider: Option<String>,
        /// Agent model (e.g., opus-4)
        #[arg(long)]
        model: Option<String>,
        /// Working directory
        #[arg(long)]
        cwd: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PluginAction {
    /// List all plugins
    List,
    /// Enable a plugin
    Enable {
        /// Plugin name or ID
        name: String,
    },
    /// Disable a plugin
    Disable {
        /// Plugin name or ID
        name: String,
    },
}

#[derive(Subcommand)]
pub enum TunnelAction {
    /// Start a tunnel
    Start {
        /// Tunnel provider (e.g., cloudflare)
        #[arg(long)]
        provider: Option<String>,
        /// Custom domain
        #[arg(long)]
        domain: Option<String>,
    },
    /// Stop the tunnel
    Stop,
    /// Show tunnel status
    Status,
}

#[derive(Subcommand)]
pub enum TaskAction {
    /// Add a new task to the queue
    Add {
        /// Task title / description
        title: String,
        /// Priority: p0, p1 (default), or p2
        #[arg(long)]
        priority: Option<String>,
        /// Working directory for the task
        #[arg(long)]
        cwd: Option<String>,
        /// Comma-separated list of task IDs this task depends on
        #[arg(long)]
        depends_on: Option<String>,
    },
    /// List all tasks
    List,
    /// Cancel a task
    Cancel {
        /// Task ID to cancel
        id: String,
    },
    /// Pause the task scheduler
    Pause,
    /// Resume the task scheduler
    Resume,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Open config in $EDITOR
    Edit,
    /// Initialize default configuration
    Init,
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
        Some(Commands::Agent { action }) => {
            init_basic_tracing();
            commands::agent::handle(action)?;
        }
        Some(Commands::Plugin { action }) => {
            init_basic_tracing();
            commands::plugin::handle(action)?;
        }
        Some(Commands::Tunnel { action }) => {
            init_basic_tracing();
            commands::tunnel::handle(action)?;
        }
        Some(Commands::Task { action }) => {
            init_basic_tracing();
            commands::task::handle(action)?;
        }
        Some(Commands::Config { action }) => {
            init_basic_tracing();
            commands::config::handle(action)?;
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
