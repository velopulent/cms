use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// CMS server and administrative command-line interface.
#[derive(Parser, Debug, Default)]
#[command(name = "cms", version, about = "Headless CMS server", long_about = None)]
pub struct Cli {
    /// Path to a config file (overrides the search path).
    #[arg(long, global = true, env = "CMS_CONFIG", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// REST API bind address, e.g. 0.0.0.0:3000 (overrides config + env).
    #[arg(long, global = true, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Database URL (overrides config + env).
    #[arg(long, global = true, value_name = "URL")]
    pub database_url: Option<String>,

    /// Log filter directive, e.g. "cms=info" (overrides config + env).
    #[arg(long, global = true, value_name = "LEVEL")]
    pub log_level: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the server (default when no subcommand is given).
    Serve,
    /// Inspect or scaffold configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Administrative operations.
    Admin {
        #[command(subcommand)]
        action: AdminAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Write a default config file (non-secret keys only).
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
        /// Write to this path instead of the default user config dir.
        #[arg(long, value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Print the effective merged configuration (secrets redacted).
    Show,
    /// Print the resolved config file path and the search order.
    Path,
}

#[derive(Subcommand, Debug)]
pub enum AdminAction {
    /// Reset a user's password.
    ResetPassword {
        #[arg(long, value_name = "USERNAME")]
        username: String,
        #[arg(long, value_name = "PASSWORD")]
        password: String,
    },
}
