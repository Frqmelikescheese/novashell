use clap::{Parser, Subcommand};

/// Nova — modular Linux desktop ricing engine
#[derive(Parser, Debug)]
#[command(
    name = "nova",
    version = "0.1.0",
    author = "Nova Contributors",
    about = "A modular, composable Linux desktop ricing engine",
    long_about = "Nova renders YAML+CSS-defined widgets on the desktop layer using GTK4 and Wayland layer shell."
)]
pub struct NovaCli {
    /// Subcommand to execute. If omitted, starts the daemon.
    #[command(subcommand)]
    pub command: Option<NovaCommand>,

    /// Path to config file (overrides default search)
    #[arg(short, long, env = "NOVA_CONFIG")]
    pub config: Option<String>,

    /// Log level: error, warn, info, debug, trace
    #[arg(short, long, default_value = "info", env = "NOVA_LOG")]
    pub log_level: String,

    /// IPC socket path override
    #[arg(long, env = "NOVA_SOCKET")]
    pub socket: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum NovaCommand {
    /// Start the NovaShell daemon (default if no subcommand given)
    Daemon {
        /// Run in foreground (do not daemonize)
        #[arg(short, long)]
        foreground: bool,
    },

    /// Reload the full configuration and re-render all widgets
    Reload,

    /// Reload only the CSS stylesheets without restarting widgets
    ReloadCss,

    /// Toggle visibility of a widget or screen by ID
    Toggle {
        /// Widget ID or screen name to toggle
        target: String,
    },

    /// Show a hidden widget or screen by ID
    Show {
        /// Widget ID or screen name to show
        target: String,
    },

    /// Hide a visible widget or screen by ID
    Hide {
        /// Widget ID or screen name to hide
        target: String,
    },

    /// Move a widget to a new position
    Move {
        /// Widget ID to move
        target: String,
        /// New X offset in pixels
        x: i32,
        /// New Y offset in pixels
        y: i32,
    },

    /// Switch to a named configuration profile
    SetProfile {
        /// Profile name to activate
        name: String,
    },

    /// List all active widgets and their states
    List,

    /// Gracefully quit the running daemon
    Quit,
}
