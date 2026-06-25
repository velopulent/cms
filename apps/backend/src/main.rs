use clap::Parser;
use cms::cli::{AdminAction, BackupAction, Cli, Command, ConfigAction, McpTransport};
use cms::config::{self, Config};
use cms::database::{connect_db_without_migrations, init_db_with_config};
use cms::middleware::auth::Actor;
use cms::repository::Repository;
use cms::services::Services;

use std::error::Error;
use std::sync::Arc;
use tracing::{debug, info};

#[tokio::main]
async fn main() {
    // Load env from the cwd `.env` (dev) first, then `$VCMS_HOME/.env` (installed
    // services + `mcp stdio`, which run from an arbitrary cwd). dotenvy is
    // first-wins, so real env vars and the cwd file keep precedence over the home
    // file. VCMS_HOME must be a real env var — resolved here before the home `.env`
    // loads — never set inside that file (chicken-and-egg).
    dotenvy::dotenv().ok();
    let home_env = cms::paths::home().join(".env");
    match dotenvy::from_path(&home_env) {
        Ok(()) => {}
        Err(e) if e.not_found() => {} // missing file is fine
        Err(e) => {
            eprintln!("Error: cannot load {}: {e}", home_env.display());
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
        }) => run_mcp_stdio(&cli).await,
        Some(Command::Serve) | None => cms::server::run(&cli, cms::server::shutdown_signal()).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run_mcp_stdio(cli: &Cli) -> Result<(), Box<dyn Error>> {
    // Read-only: never create the home dir, database, or secrets file here. The
    // persisted secrets written by `vcms serve` are what make this process verify
    // the site token with the same HMAC secret the server signed it with.
    if cms::secrets::load()?.is_none() && std::env::var("HMAC_SECRET").is_err() {
        return Err("No instance secrets found. Run `vcms serve` once to initialize \
                    ~/.vcms (or set HMAC_SECRET) before `vcms mcp stdio`."
            .into());
    }

    let config = Config::load(cli)?;
    cms::tracing::init_stdio_tracing(&config);

    info!("Starting standalone MCP stdio process");
    debug!(
        database_backend = ?cms::database::backend::DatabaseBackend::from_url(&config.database_url),
        "MCP stdio configuration loaded"
    );

    if let Err(error) = config.validate_security() {
        return Err(format!("Invalid production security configuration: {error}").into());
    }

    let token = std::env::var("VCMS_MCP_TOKEN").map_err(|_| "VCMS_MCP_TOKEN is required for `vcms mcp stdio`")?;
    if token.trim().is_empty() {
        return Err("VCMS_MCP_TOKEN must not be empty".into());
    }

    let pool = connect_db_without_migrations(&config).await?;
    info!("Existing CMS database schema is compatible; no migrations were run");

    let repository = Repository::new(&pool);
    let actor = cms::mcp::auth::verify_stdio_token(&token, &repository, &config.hmac_secret)
        .await
        .map_err(|error| format!("MCP stdio authentication failed: {}", error.message))?;
    match &actor {
        Actor::ApiKey(api_key) => info!(
            site_id = %api_key.site_id,
            permission = %api_key.permission,
            "MCP stdio site token authenticated"
        ),
        Actor::User(_) => return Err("MCP stdio requires a CMS site access token".into()),
    }

    let storage_registry = cms::server::initialize_storage(&config);
    let repository = Arc::new(repository);
    let config = Arc::new(config);
    // Read-only search: stdio can run alongside the server without contending for
    // the writer lock; its content writes enqueue for the server to index.
    let services = Arc::new(Services::new_read_only(repository.clone(), &pool, &config));
    let server = cms::mcp::server::CmsServer::new_stdio(services, repository, storage_registry, config, token);

    let result = cms::mcp::transports::stdio::serve(server).await;
    match &result {
        Ok(()) => info!("Standalone MCP stdio process exited cleanly"),
        Err(error) => tracing::error!(error = %error, "Standalone MCP stdio process exiting after failure"),
    }
    result
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
