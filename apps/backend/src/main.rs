use clap::Parser;
use cms::cli::{AdminAction, BackupAction, Cli, Command, ConfigAction, McpTransport};
use cms::config::{self, Config};
use cms::database::{connect_db_without_migrations, init_db_with_config};
use cms::grpc::server::spawn_grpc_server;
use cms::middleware::auth::Actor;
use cms::repository::Repository;
use cms::router::create_router;
use cms::services::Services;
use cms::storage::{self, STORAGE_KIND_FILESYSTEM, STORAGE_KIND_S3, StorageRegistry};

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, info, warn};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

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
        Some(Command::Mcp {
            transport: McpTransport::Stdio,
        }) => run_mcp_stdio(&cli).await,
        Some(Command::Serve) | None => run_serve(&cli).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run_serve(cli: &Cli) -> Result<(), Box<dyn Error>> {
    cms::paths::ensure()?;
    cms::secrets::ensure()?;
    let config = Config::load(cli)?;

    let _guard = cms::tracing::init_tracing(&config);

    if let Err(e) = config.validate_security() {
        return Err(format!("Invalid production security configuration: {e}").into());
    }

    let pool = init_db_with_config(&config).await?;

    let repository = Repository::new(&pool);

    seed_admin(&repository).await;

    let storage_registry = initialize_storage(&config);
    // Build these once and share them: the REST router and the gRPC server use the
    // same `Services` so the single-writer search index is opened only once.
    let repository_arc = Arc::new(repository.clone());
    let config_arc = Arc::new(config.clone());
    let services = Services::new(repository_arc.clone(), &pool, &config);

    let backup_destination = cms::services::backup::build_backup_destination(&config)
        .map_err(|e| format!("Failed to initialize backup destination: {e}"))?;
    let backup_service = Arc::new(cms::services::backup::BackupService::new(
        pool.clone(),
        storage_registry.clone(),
        backup_destination,
        &config,
    ));

    let app = create_router(
        repository.clone(),
        config.clone(),
        storage_registry.clone(),
        services.clone(),
        backup_service.clone(),
    );

    // Reconcile backups/restore jobs left mid-flight by a previous process: any
    // running/pending row at startup is orphaned (backups only run in-process).
    match cms::services::backup::meta::fail_orphaned(&pool, &cms::services::backup::now_iso()).await {
        Ok(n) if n > 0 => info!("Reconciled {n} interrupted backup/restore job(s) to failed"),
        Ok(_) => {}
        Err(e) => tracing::error!("Failed to reconcile interrupted backups: {e}"),
    }

    if config.backup_enabled {
        let scheduler_service = backup_service.clone();
        tokio::spawn(async move {
            cms::services::backup::scheduler::run(scheduler_service).await;
        });
        info!("Backup scheduler started");
    }

    // The search indexer is the single writer/consumer: it rebuilds the index when
    // empty (first run / wiped), then drains the cross-process queue forever.
    if let (Some(search), Some(queue)) = (services.search.clone(), services.search_queue.clone()) {
        let repo = repository_arc.clone();
        tokio::spawn(async move {
            if search.is_empty() {
                info!("Search index is empty; building from database...");
                match search.rebuild_all(&repo).await {
                    Ok(n) => info!("Search index built: {} entries", n),
                    Err(e) => tracing::error!("Search index build failed: {}", e),
                }
            }
            cms::services::search::indexer::run(search, queue, repo).await;
        });
    }

    let addr: SocketAddr = config.bind_address.parse().expect("Invalid BIND_ADDRESS");
    info!("REST API server running on {}", addr);

    let grpc_addr: SocketAddr = config.grpc_bind_address.parse().expect("Invalid GRPC_BIND_ADDRESS");
    info!("gRPC server running on {}", grpc_addr);

    let rest_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind address");

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
    });

    let grpc_handle = tokio::spawn(spawn_grpc_server(
        services.clone(),
        repository_arc.clone(),
        config_arc.clone(),
        storage_registry.clone(),
        grpc_addr,
    ));

    tokio::select! {
        result = rest_handle => {
            if let Err(e) = result {
                tracing::error!("REST server error: {}", e);
            }
        }
        result = grpc_handle => {
            if let Err(e) = result {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    }

    Ok(())
}

async fn run_mcp_stdio(cli: &Cli) -> Result<(), Box<dyn Error>> {
    // Read-only: never create the home dir, database, or secrets file here. The
    // persisted secrets written by `cms serve` are what make this process verify
    // the site token with the same HMAC secret the server signed it with.
    if cms::secrets::load()?.is_none() && std::env::var("HMAC_SECRET").is_err() {
        return Err("No instance secrets found. Run `cms serve` once to initialize \
                    ~/.cms (or set HMAC_SECRET) before `cms mcp stdio`."
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

    let token = std::env::var("CMS_MCP_TOKEN").map_err(|_| "CMS_MCP_TOKEN is required for `cms mcp stdio`")?;
    if token.trim().is_empty() {
        return Err("CMS_MCP_TOKEN must not be empty".into());
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

    let storage_registry = initialize_storage(&config);
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

fn initialize_storage(config: &Config) -> Arc<StorageRegistry> {
    let mut storage_registry = StorageRegistry::new();

    // Use an explicit filesystem path if set; otherwise default to ~/.cms/storage
    // so uploads work out of the box — unless S3 is configured and takes over.
    let fs_path = match (&config.storage_fs_path, config.has_s3()) {
        (Some(path), _) => Some(path.clone()),
        (None, false) => Some(cms::paths::storage_dir().to_string_lossy().into_owned()),
        (None, true) => None,
    };

    if let Some(fs_path) = fs_path {
        match storage::FileSystemStorage::new(&fs_path) {
            Ok(fs) => {
                storage_registry.register(STORAGE_KIND_FILESYSTEM, Arc::new(fs));
                info!("Filesystem storage initialized at {}", fs_path);
            }
            Err(error) => warn!("Failed to initialize filesystem storage: {}", error),
        }
    }

    if config.has_s3() {
        match storage::S3Storage::new(
            config.s3_access_key_id.as_deref().unwrap(),
            config.s3_secret_access_key.as_deref().unwrap(),
            config.s3_bucket.as_deref().unwrap(),
            config.s3_region.as_deref().unwrap_or("us-east-1"),
            config.s3_endpoint.as_deref(),
            config.s3_public_url.as_deref(),
        ) {
            Ok(s3) => {
                storage_registry.register(STORAGE_KIND_S3, Arc::new(s3));
                info!("S3 storage initialized");
            }
            Err(error) => warn!("Failed to initialize S3 storage: {}", error),
        }
    }

    if storage_registry.get(STORAGE_KIND_FILESYSTEM).is_none() && storage_registry.get(STORAGE_KIND_S3).is_none() {
        warn!("No storage providers configured. Set STORAGE_FS_PATH or S3_* env vars.");
    }

    Arc::new(storage_registry)
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
                Some(explicit) => println!("  1. --config / CMS_CONFIG: {}", explicit.display()),
                None => println!("  1. --config / CMS_CONFIG: <not set>"),
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
        AdminAction::ResetPassword { username, password } => {
            let id = match repository.user.find_id_by_username(username).await? {
                Some(id) => id,
                None => return Err(format!("User '{username}' not found").into()),
            };
            let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
            repository.user.update_password(&id, &password_hash, false).await?;
            println!("Password updated for user '{username}'.");
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
    let storage_registry = initialize_storage(&config);
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
    let storage_registry = initialize_storage(&config);
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

async fn seed_admin(repository: &Repository) {
    debug!("Checking if admin user needs to be seeded");
    if !repository.user.exists("admin").await.unwrap_or(false) {
        info!("Seeding default admin user");
        let id = uuid::Uuid::now_v7().to_string();
        let password_hash = bcrypt::hash("admin", bcrypt::DEFAULT_COST).expect("Failed to hash password");
        repository
            .user
            .create(&id, "admin", "admin@cms.local", &password_hash)
            .await
            .expect("Failed to seed admin user");
        repository
            .user
            .set_instance_role(&id, Some("instance_owner"))
            .await
            .expect("Failed to assign instance owner");
        repository
            .user
            .update_password(&id, &password_hash, true)
            .await
            .expect("Failed to require password change");

        warn!("Seeded default admin user (admin/admin) — CHANGE THE PASSWORD IMMEDIATELY!");
        eprintln!(
            "\n\
             ============================ SECURITY WARNING ============================\n\
             A default admin account was created:  username 'admin'  password 'admin'\n\
             Anyone who can reach this server can log in until you change it. Run:\n\
             \n    cms admin reset-password --username admin --password <new-strong-password>\n\n\
             or change it from the dashboard now. Do NOT expose this server until done.\n\
             =========================================================================\n"
        );
    } else {
        debug!("Admin user already exists, skipping seeding");
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down..."),
        _ = terminate => info!("Received SIGTERM, shutting down..."),
    }
}
