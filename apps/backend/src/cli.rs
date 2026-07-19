use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Default)]
#[command(
    name = "vcms",
    version,
    about = "Velopulent CMS",
    long_about = "Velopulent CMS — self-hosted headless content management system.\n\n`vcms serve` runs a portable instance from ./vcms_data. When the native service is installed, operational commands use its fixed system data root.",
    after_help = "DOCUMENTATION:\n  https://cms.velopulent.com/docs"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run a portable server from ./vcms_data.
    Serve,
    /// Run Model Context Protocol transports.
    Mcp {
        #[command(subcommand)]
        transport: McpTransport,
    },
    /// Inspect effective configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage the restricted instance trust root.
    Secrets {
        #[command(subcommand)]
        action: SecretsAction,
    },
    /// Administrative operations.
    Admin {
        #[command(subcommand)]
        action: AdminAction,
    },
    /// Create or list backups.
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
    /// Restore a backup artifact (destructive).
    Restore {
        #[arg(long, value_name = "PATH")]
        file: PathBuf,
        #[arg(long, default_value = "instance", value_name = "SCOPE")]
        scope: String,
        #[arg(long, value_name = "SITE_ID")]
        site: Option<String>,
        #[arg(long)]
        import_as_new: bool,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum BackupAction {
    Create {
        #[arg(long, default_value = "instance", value_name = "SCOPE")]
        scope: String,
        #[arg(long, value_name = "SITE_ID")]
        site: Option<String>,
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        #[arg(long = "no-files")]
        no_files: bool,
        #[arg(long)]
        encrypt: bool,
    },
    List,
}

#[derive(Subcommand, Debug)]
pub enum ServiceAction {
    /// Print normalized native service state and manager details.
    Status,
    /// Internal native-service entry point. Not for direct use.
    #[command(hide = true)]
    Run,
}

#[derive(Subcommand, Debug)]
pub enum McpTransport {
    Stdio,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    Show,
}

#[derive(Subcommand, Debug)]
pub enum SecretsAction {
    Reset {
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum AdminAction {
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
    fn no_subcommand_is_help_mode() {
        assert!(Cli::try_parse_from(["vcms"]).unwrap().command.is_none());
    }

    #[test]
    fn parses_operational_commands() {
        let mcp = Cli::try_parse_from(["vcms", "mcp", "stdio"]).unwrap();
        assert!(matches!(
            mcp.command,
            Some(Command::Mcp {
                transport: McpTransport::Stdio
            })
        ));
        let service = Cli::try_parse_from(["vcms", "service", "run"]).unwrap();
        assert!(matches!(
            service.command,
            Some(Command::Service {
                action: ServiceAction::Run
            })
        ));
    }
}
