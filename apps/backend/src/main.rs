use clap::Parser;
use cms::cli::{AdminAction, BackupAction, Cli, Command, ConfigAction, McpTransport};
use cms::config::{self, Config};
use cms::database::init_db_with_config;
use cms::repository::Repository;

use std::error::Error;
use tracing::info;

#[tokio::main]
async fn main() {
    // Load env from the cwd `.env` (dev) first, then `$VCMS_HOME/.env` (installed
    // services run from an arbitrary cwd). dotenvy is first-wins, so real env vars and
    // the cwd file keep precedence over the config-dir file. VCMS_HOME must be a real
    // env var — resolved here before the `.env` loads — never set inside that file
    // (chicken-and-egg).
    dotenvy::dotenv().ok();
    let env_file = cms::paths::env_file();
    match dotenvy::from_path(&env_file) {
        Ok(()) => {}
        Err(e) if e.not_found() => {} // missing file is fine
        Err(e) => {
            eprintln!("Error: cannot load {}: {e}", env_file.display());
            std::process::exit(1);
        }
    }

    let cli = Cli::parse();

    let result = match &cli.command {
        Some(Command::Config { action }) => run_config(action, &cli),
        Some(Command::Admin { action }) => run_admin(action, &cli).await,
        Some(Command::Backup { action }) => run_backup(action, &cli).await,
        Some(Command::Restore {
            file,
            scope,
            site,
            import_as_new,
            yes,
        }) => run_restore(file, scope, site, *import_as_new, *yes, &cli).await,
        Some(Command::Service { action }) => cms::service::run_service(action, &cli).await,
        Some(Command::Mcp {
            transport: McpTransport::Stdio,
        }) => run_mcp_stdio().await,
        Some(Command::Serve) | None => cms::server::run(&cli, cms::server::shutdown_signal()).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run_mcp_stdio() -> Result<(), Box<dyn Error>> {
    // A thin proxy to the running server's `/mcp` endpoint — it touches no disk
    // (no home dir, database, secrets, or search index), so it works even when those
    // belong to the privileged service account. It needs only a server URL and a
    // `vcms_site_*` access token, forwarded as the bearer credential.
    cms::tracing::init_proxy_tracing();

    let token = std::env::var("VCMS_MCP_TOKEN").map_err(|_| "VCMS_MCP_TOKEN is required for `vcms mcp stdio`")?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("VCMS_MCP_TOKEN must not be empty".into());
    }

    let base = std::env::var("VCMS_MCP_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
    let endpoint = format!("{}/mcp", base.trim_end_matches('/'));

    info!(%endpoint, "Starting MCP stdio proxy");
    cms::mcp::transports::stdio::serve(endpoint, token).await
}

fn run_config(action: &ConfigAction, cli: &Cli) -> Result<(), Box<dyn Error>> {
    match action {
        ConfigAction::Init { force, path } => {
            let target = match path {
                Some(p) => p.clone(),
                None => config::user_config_path().ok_or("Could not determine user config directory")?,
            };
            if target.exists() && !force {
                return Err(format!("{} already exists (use --force to overwrite)", target.display()).into());
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, config::default_config_toml())?;
            println!("Wrote default config to {}", target.display());
        }
        ConfigAction::Show => {
            let config = Config::load(cli)?;
            print!("{}", config.redacted_toml());
        }
        ConfigAction::Path => {
            match config::resolve_config_path(cli) {
                Some(p) => println!("Active config file: {}", p.display()),
                None => println!("Active config file: <none found; using built-in defaults>"),
            }
            println!("\nSearch order (first existing wins):");
            match &cli.config {
                Some(explicit) => println!("  1. --config / VCMS_CONFIG: {}", explicit.display()),
                None => println!("  1. --config / VCMS_CONFIG: <not set>"),
            }
            for (i, p) in config::config_search_paths().iter().enumerate() {
                let marker = if p.exists() { " (exists)" } else { "" };
                println!("  {}. {}{}", i + 2, p.display(), marker);
            }
        }
    }
    Ok(())
}

async fn run_admin(action: &AdminAction, cli: &Cli) -> Result<(), Box<dyn Error>> {
    cms::paths::ensure()?;
    cms::secrets::ensure()?;
    let config = Config::load(cli)?;
    let pool = init_db_with_config(&config).await?;
    let repository = Repository::new(&pool);

    match action {
        AdminAction::ResetPassword { email, password } => {
            let id = match repository.user.find_by_email(email).await? {
                Some(user) => user.id,
                None => return Err(format!("User with email '{email}' not found").into()),
            };
            let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
            repository.user.update_password(&id, &password_hash, false).await?;
            println!("Password updated for user with email '{email}'.");
        }
    }
    Ok(())
}

async fn run_backup(action: &BackupAction, cli: &Cli) -> Result<(), Box<dyn Error>> {
    use cms::services::backup::{BackupService, CreateBackupOptions, build_backup_destination, meta};

    cms::paths::ensure()?;
    cms::secrets::ensure()?;
    let config = Config::load(cli)?;
    let pool = init_db_with_config(&config).await?;
    let storage_registry = cms::server::initialize_storage(&config);
    let destination = build_backup_destination(&config)?;
    let service = BackupService::new(pool.clone(), storage_registry, destination, &config);

    match action {
        BackupAction::Create {
            scope,
            site,
            out,
            no_files,
            encrypt,
        } => {
            let scope = parse_scope(scope, site.as_deref())?;
            let include_files = !no_files;
            if let Some(out) = out {
                let (manifest, bytes) = service.build_artifact(&scope, include_files, *encrypt).await?;
                std::fs::write(out, &bytes)?;
                println!(
                    "Wrote backup to {} ({} bytes, {} tables, {} files)",
                    out.display(),
                    bytes.len(),
                    manifest.tables.len(),
                    manifest.files.len()
                );
            } else {
                let row = service
                    .create_backup(CreateBackupOptions {
                        scope,
                        include_files,
                        encrypt: *encrypt,
                        schedule_id: None,
                        created_by: None,
                    })
                    .await?;
                println!(
                    "Backup {} created ({} bytes) -> {}",
                    row.id,
                    row.size_bytes,
                    row.destination_key.unwrap_or_default()
                );
            }
        }
        BackupAction::List => {
            let rows = meta::list_backups(service.pool(), None, None).await?;
            if rows.is_empty() {
                println!("No backups recorded.");
            }
            for r in rows {
                println!(
                    "{}  {:8}  scope={:8} site={:36}  {}  {} bytes  {}",
                    r.id,
                    r.status,
                    r.scope,
                    r.site_id.unwrap_or_else(|| "-".into()),
                    if r.encrypted != 0 { "encrypted" } else { "plaintext" },
                    r.size_bytes,
                    r.created_at
                );
            }
        }
    }
    Ok(())
}

async fn run_restore(
    file: &std::path::Path,
    scope: &str,
    site: &Option<String>,
    import_as_new: bool,
    yes: bool,
    cli: &Cli,
) -> Result<(), Box<dyn Error>> {
    use cms::services::backup::{
        BackupService, RestoreRequest, RestoreSource, RestoreTarget, build_backup_destination,
    };

    if !yes {
        return Err("Restore is destructive and replaces data within the chosen scope. \
                    Re-run with --yes to proceed."
            .into());
    }

    cms::paths::ensure()?;
    cms::secrets::ensure()?;
    let config = Config::load(cli)?;
    let pool = init_db_with_config(&config).await?;
    let storage_registry = cms::server::initialize_storage(&config);
    let destination = build_backup_destination(&config)?;
    let service = BackupService::new(pool.clone(), storage_registry, destination, &config);

    let bytes = std::fs::read(file)?;
    let target = match scope {
        "instance" => RestoreTarget::WholeInstance,
        "site" => {
            let sid = site.clone().ok_or("--site <SITE_ID> is required for --scope site")?;
            RestoreTarget::Site {
                site_id: sid,
                import_as_new,
            }
        }
        other => return Err(format!("unknown scope '{other}' (use instance|site)").into()),
    };

    service
        .restore(RestoreRequest {
            source: RestoreSource::Bytes(bytes),
            target,
            created_by: None,
        })
        .await?;
    println!("Restore complete.");
    let reindex_path = match (scope, site) {
        ("site", Some(sid)) => format!("/api/dashboard/sites/{sid}/search/reindex"),
        _ => "/api/dashboard/instance/search/reindex".to_string(),
    };
    println!(
        "Note: the full-text search index may now be stale. Rebuild it from the dashboard \
         (Settings → Backups → Rebuild search index) or POST {reindex_path} once the server is running."
    );
    Ok(())
}

fn parse_scope(scope: &str, site: Option<&str>) -> Result<cms::services::backup::Scope, Box<dyn Error>> {
    use cms::services::backup::Scope;
    match scope {
        "instance" => Ok(Scope::Instance),
        "site" => site
            .map(|s| Scope::Site(s.to_string()))
            .ok_or_else(|| "--site <SITE_ID> is required for --scope site".into()),
        other => Err(format!("unknown scope '{other}' (use instance|site)").into()),
    }
}
