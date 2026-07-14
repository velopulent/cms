//! Server bootstrap shared by `vcms serve` and the platform service runners.
//!
//! The full startup sequence (home dir, secrets, config, migrations, REST + gRPC)
//! lives here rather than in `main.rs` so that the Windows Service Control Manager
//! entry point — which lives in the library — can host the exact same server. The
//! caller injects a *shutdown future*; whatever resolves it (Ctrl+C / SIGTERM on a
//! normal run, an SCM stop control on Windows) triggers one graceful drain of both
//! the REST and gRPC servers.

use std::error::Error;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::cli::Cli;
use crate::config::Config;
use crate::database::init_db_with_config;
use crate::grpc::server::spawn_grpc_server;
use crate::repository::Repository;
use crate::router::create_router;
use crate::services::Services;
use crate::storage::{self, STORAGE_KIND_FILESYSTEM, STORAGE_KIND_S3, StorageRegistry};

/// Email identity of the default admin account. Login is by email, so the seed
/// gate keys on this (non-unique display name "admin" would be the wrong column).
const ADMIN_EMAIL: &str = "admin@cms.local";

/// Boot the full server and run until `shutdown` resolves.
///
/// Shared by the foreground `serve` command and the OS service runners. The
/// `shutdown` future is the single trigger that gracefully drains both the REST
/// and gRPC listeners. `on_ready` fires once startup has actually succeeded —
/// database open + migrated and both listeners bound — so a service host (the
/// Windows SCM runner) can report `Running` truthfully instead of optimistically.
pub async fn run(
    cli: &Cli,
    shutdown: impl Future<Output = ()> + Send + 'static,
    on_ready: impl FnOnce(),
) -> Result<(), Box<dyn Error>> {
    // Each early step gets a context prefix: a bare io error ("Access is denied.
    // (os error 5)") from a service host is undebuggable without knowing which
    // file/step produced it.
    crate::paths::ensure().map_err(|e| format!("preparing data directories: {e}"))?;
    crate::secrets::ensure().map_err(|e| format!("initializing secrets.toml: {e}"))?;
    let config = Config::load(cli).map_err(|e| format!("loading configuration: {e}"))?;

    let _guard = crate::tracing::init_tracing(&config);

    if let Err(e) = config.validate_security() {
        return Err(format!("Invalid production security configuration: {e}").into());
    }

    let pool = init_db_with_config(&config)
        .await
        .map_err(|e| format!("opening database: {e}"))?;

    let repository = Repository::new(&pool);

    seed_admin(&repository).await;

    let storage_registry = initialize_storage(&config);
    // Build these once and share them: the REST router and the gRPC server use the
    // same `Services` so the single-writer search index is opened only once.
    let repository_arc = Arc::new(repository.clone());
    let config_arc = Arc::new(config.clone());
    let services = Services::new(repository_arc.clone(), &pool, &config);

    let backup_destination = crate::services::backup::build_backup_destination(&config)
        .map_err(|e| format!("Failed to initialize backup destination: {e}"))?;
    let backup_service = Arc::new(crate::services::backup::BackupService::new(
        pool.clone(),
        storage_registry.clone(),
        backup_destination,
        &config,
    ));

    let app = create_router(
        pool.clone(),
        repository.clone(),
        config.clone(),
        storage_registry.clone(),
        services.clone(),
        backup_service.clone(),
    );

    // Reconcile backups/restore jobs left mid-flight by a previous process: any
    // running/pending row at startup is orphaned (backups only run in-process).
    match crate::services::backup::meta::fail_orphaned(&pool, &crate::services::backup::now_iso()).await {
        Ok(n) if n > 0 => info!("Reconciled {n} interrupted backup job(s) to failed"),
        Ok(_) => {}
        Err(e) => tracing::error!("Failed to reconcile interrupted backups: {e}"),
    }

    if config.backup_enabled {
        let scheduler_service = backup_service.clone();
        tokio::spawn(async move {
            crate::services::backup::scheduler::run(scheduler_service).await;
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
            crate::services::search::indexer::run(search, queue, repo).await;
        });
    }

    let addr: SocketAddr = config
        .bind_address
        .parse()
        .map_err(|e| format!("Invalid BIND_ADDRESS '{}': {e}", config.bind_address))?;
    info!("Dashboard UI available at http://{}/dashboard", addr);
    info!("REST API server running on http://{}", addr);
    info!("GraphQL endpoint at http://{}/api/graphql", addr);
    if config.mcp_enabled {
        info!("MCP HTTP endpoint at http://{}/mcp", addr);
    }

    let grpc_addr: SocketAddr = config
        .grpc_bind_address
        .parse()
        .map_err(|e| format!("Invalid GRPC_BIND_ADDRESS '{}': {e}", config.grpc_bind_address))?;
    info!("gRPC server running on {}", grpc_addr);

    // Bind both listeners *before* declaring readiness (and before the serve loops
    // spawn): a bind failure — the classic "port already taken" — must surface as a
    // startup error, not as a background task that dies after we claimed to be up.
    let rest_listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind REST address {addr}: {e}"))?;
    let grpc_listener = tokio::net::TcpListener::bind(grpc_addr)
        .await
        .map_err(|e| format!("Failed to bind gRPC address {grpc_addr}: {e}"))?;

    on_ready();

    // One shutdown signal fans out to both listeners via a watch channel: the
    // injected `shutdown` future flips it, and both servers drain on the change.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let shutdown_tx2 = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown.await;
        let _ = shutdown_tx.send(true);
    });

    let rest_rx = shutdown_rx.clone();
    let mut rest_handle = tokio::spawn(async move {
        axum::serve(
            rest_listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(wait_for_shutdown(rest_rx))
        .await
        .map_err(|e| {
            tracing::error!("REST server error: {e}");
            e
        })
    });

    let mut grpc_handle = tokio::spawn(spawn_grpc_server(
        services.clone(),
        repository_arc.clone(),
        config_arc.clone(),
        storage_registry.clone(),
        grpc_listener,
        Box::pin(wait_for_shutdown(shutdown_rx)),
    ));

    // Whichever server finishes first flips the shutdown signal; the sibling is
    // then awaited so both are fully drained before `run()` returns.
    let (rest_result, grpc_result) = tokio::select! {
        result = &mut rest_handle => {
            let _ = shutdown_tx2.send(true);
            let grpc_result = grpc_handle.await;
            (result, grpc_result)
        }
        result = &mut grpc_handle => {
            let _ = shutdown_tx2.send(true);
            let rest_result = rest_handle.await;
            (rest_result, result)
        }
    };

    match rest_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e.into()),
        Err(e) => return Err(e.into()),
    }
    match grpc_result {
        Ok(Ok(())) => {}
        // gRPC's error is `Box<dyn Error + Send + Sync>`; format it so the
        // conversion to this fn's `Box<dyn Error>` is unambiguous.
        Ok(Err(e)) => return Err(format!("gRPC server error: {e}").into()),
        Err(e) => return Err(format!("gRPC server task panicked: {e}").into()),
    }

    Ok(())
}

/// Resolve once the watch channel reports a shutdown was requested.
async fn wait_for_shutdown(mut rx: tokio::sync::watch::Receiver<bool>) {
    if *rx.borrow_and_update() {
        return;
    }
    let _ = rx.changed().await;
}

/// Wait for a Ctrl+C or (on unix) SIGTERM — the foreground `serve` shutdown trigger.
pub async fn shutdown_signal() {
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

/// Register the configured storage backends (filesystem and/or S3).
pub fn initialize_storage(config: &Config) -> Arc<StorageRegistry> {
    let mut storage_registry = StorageRegistry::new();

    // Use an explicit filesystem path if set; otherwise default to the data dir's
    // storage/ so uploads work out of the box — unless S3 is configured and takes over.
    let fs_path = match (&config.storage_fs_path, config.has_s3()) {
        (Some(path), _) => Some(path.clone()),
        (None, false) => Some(crate::paths::storage_dir().to_string_lossy().into_owned()),
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

async fn seed_admin(repository: &Repository) {
    debug!("Checking if admin user needs to be seeded");
    let exists = match repository.user.exists(ADMIN_EMAIL).await {
        Ok(exists) => exists,
        Err(e) => {
            tracing::error!("Failed to check for existing admin user; skipping seed: {e}");
            return;
        }
    };
    if !exists {
        info!("Seeding default admin user");
        let id = uuid::Uuid::now_v7().to_string();
        let password_hash = bcrypt::hash("admin", bcrypt::DEFAULT_COST).expect("Failed to hash password");
        repository
            .user
            .create(&id, "admin", ADMIN_EMAIL, &password_hash)
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

        warn!("Seeded default admin user (admin@cms.local / admin) — CHANGE THE PASSWORD IMMEDIATELY!");
        eprintln!(
            "\n\
             ============================ SECURITY WARNING ============================\n\
             A default admin account was created:  email 'admin@cms.local'  password 'admin'\n\
             Sign in with the email and password. Anyone who can reach this server can log\n\
             in until you change it. Run:\n\
             \n    vcms admin reset-password --email admin@cms.local --password <new-strong-password>\n\n\
             or change it from the dashboard now. Do NOT expose this server until done.\n\
             =========================================================================\n"
        );
    } else {
        debug!("Admin user already exists, skipping seeding");
    }
}
