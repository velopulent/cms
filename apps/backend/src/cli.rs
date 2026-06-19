use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// CMS server and administrative command-line interface.
#[derive(Parser, Debug, Default)]
#[command(
    name = "cms",
    version,
    about = "Headless CMS server",
    long_about = "Headless CMS server.\n\n\
        All runtime files live under one home directory ($CMS_HOME, default ~/.cms): \
        config.toml, secrets.toml, the SQLite database, logs/, and storage/. \
        `cms serve` creates it on first run and generates secrets if absent.",
    after_help = "DATA DIRECTORY ($CMS_HOME, default ~/.cms):\n  \
        config.toml   non-secret config (written by `cms config init`)\n  \
        secrets.toml  auto-generated HMAC + backup secrets (0600 on unix)\n  \
        cms.db        default SQLite database (+ -wal / -shm)\n  \
        logs/         rolling logs when [log] output = \"file\"\n  \
        storage/      default filesystem storage for uploads\n\n\
        KEY ENVIRONMENT (env overrides config; CLI flags override env):\n  \
        CMS_HOME      home directory                   [default: ~/.cms]\n  \
        DATABASE_URL  sqlite/postgres/mysql URL        [default: sqlite://~/.cms/cms.db]\n  \
        HMAC_SECRET   token-lookup HMAC key            [auto-generated to secrets.toml]"
)]
pub struct Cli {
    /// Path to a config file (overrides the search path).
    #[arg(long, global = true, env = "CMS_CONFIG", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// REST API bind address, e.g. 0.0.0.0:3000 (overrides config + env).
    #[arg(long, global = true, value_name = "ADDR")]
    pub bind: Option<String>,

    /// Database URL, e.g. sqlite:path / postgres://… (overrides config + env)
    /// [default: sqlite://~/.cms/cms.db].
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
    /// Reset a user's password.
    ResetPassword {
        #[arg(long, value_name = "USERNAME")]
        username: String,
        #[arg(long, value_name = "PASSWORD")]
        password: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, McpTransport};
    use clap::Parser;

    #[test]
    fn parses_mcp_stdio_command() {
        let cli = Cli::try_parse_from(["cms", "mcp", "stdio"]).expect("command should parse");
        assert!(matches!(
            cli.command,
            Some(Command::Mcp {
                transport: McpTransport::Stdio
            })
        ));
    }
}
