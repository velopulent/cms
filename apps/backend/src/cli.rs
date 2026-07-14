use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// CMS server and administrative command-line interface.
#[derive(Parser, Debug, Default)]
#[command(
    name = "vcms",
    version,
    about = "Velopulent CMS",
    long_about = "Velopulent CMS — self-hosted headless content management system.\n\n\
        Runtime files default to the platform's per-type directories (config, data, \
        cache, state) via the `directories` crate. Set $VCMS_HOME to keep everything \
        under one root instead. OS packages set it from their service definitions. \
        `vcms serve` creates what it needs on first run and generates secrets if absent.",
    after_help = "DATA DIRECTORIES (defaults; set $VCMS_HOME for a single root):\n  \
        config dir   config.toml, secrets.toml (0600 on unix), .env\n  \
        data dir     vcms.db (+ -wal / -shm), storage/, backups/\n  \
        cache dir    search/ (derived Tantivy index, rebuildable)\n  \
        state dir    logs/ (when [log] output = \"file\")\n\n\
        KEY ENVIRONMENT (env overrides config; CLI flags override env):\n  \
        VCMS_HOME     force single-root layout         [default: platform split dirs]\n  \
        DATABASE_URL  sqlite/postgres/mysql URL        [default: sqlite://<data dir>/vcms.db]\n  \
        HMAC_SECRET   token-lookup HMAC key            [auto-generated to secrets.toml]\n\n\
        DOCUMENTATION:\n  \
        https://cms.velopulent.com/docs"
)]
pub struct Cli {
    /// Path to a config file (overrides the search path).
    #[arg(long, global = true, env = "VCMS_CONFIG", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// REST API bind address, e.g. 0.0.0.0:3000 (overrides config + env).
    #[arg(long, global = true, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Database URL, e.g. sqlite:path / postgres://… (overrides config + env)
    /// [default: sqlite://<data dir>/vcms.db].
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
    /// Run Model Context Protocol transports.
    Mcp {
        #[command(subcommand)]
        transport: McpTransport,
    },
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
    /// Create or list backups (runs offline, without the HTTP server).
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },
    /// Inspect the native vcms service.
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Validate configuration, storage, database, ports, and service context.
    Doctor,
    /// Restore a backup artifact (runs offline; destructive — replaces data in scope).
    Restore {
        /// Path to the backup artifact (`.cmsbak`).
        #[arg(long, value_name = "PATH")]
        file: PathBuf,
        /// Restore scope: `instance` (whole instance) or `site` (a single site).
        #[arg(long, default_value = "instance", value_name = "SCOPE")]
        scope: String,
        /// Site id to restore (required when --scope site, or to extract one site
        /// from an instance backup).
        #[arg(long, value_name = "SITE_ID")]
        site: Option<String>,
        /// Import the site under fresh ids instead of replacing in place.
        #[arg(long)]
        import_as_new: bool,
        /// Skip the destructive-action confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BackupAction {
    /// Create a backup.
    Create {
        /// `instance` (everything) or `site` (one site).
        #[arg(long, default_value = "instance", value_name = "SCOPE")]
        scope: String,
        /// Site id (required when --scope site).
        #[arg(long, value_name = "SITE_ID")]
        site: Option<String>,
        /// Write the artifact to this file instead of the configured destination.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// Exclude uploaded files from the backup.
        #[arg(long = "no-files")]
        no_files: bool,
        /// Encrypt the artifact (requires a backup encryption key).
        #[arg(long)]
        encrypt: bool,
    },
    /// List recorded backups.
    List,
}

#[derive(Subcommand, Debug)]
pub enum ServiceAction {
    /// Print normalized native service state and manager details.
    Status,
    /// Internal entry point invoked by the Windows Service Control Manager.
    ///
    /// Not for direct use — the SCM launches this to host the server inside a
    /// Windows service. Hidden from `--help`.
    #[command(hide = true)]
    #[cfg(windows)]
    Run,
}

#[derive(Subcommand, Debug)]
pub enum McpTransport {
    /// Run MCP over stdin/stdout.
    Stdio,
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
    /// Reset a user's password. The user is identified by their unique login email.
    ResetPassword {
        #[arg(long, value_name = "EMAIL")]
        email: String,
        #[arg(long, value_name = "PASSWORD")]
        password: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, McpTransport, ServiceAction};
    use clap::Parser;

    #[test]
    fn parses_mcp_stdio_command() {
        let cli = Cli::try_parse_from(["vcms", "mcp", "stdio"]).expect("command should parse");
        assert!(matches!(
            cli.command,
            Some(Command::Mcp {
                transport: McpTransport::Stdio
            })
        ));
    }

    #[test]
    fn parses_operational_commands() {
        let doctor = Cli::try_parse_from(["vcms", "doctor"]).unwrap();
        assert!(matches!(doctor.command, Some(Command::Doctor)));

        let status = Cli::try_parse_from(["vcms", "service", "status"]).unwrap();
        assert!(matches!(
            status.command,
            Some(Command::Service {
                action: ServiceAction::Status
            })
        ));
    }
}
