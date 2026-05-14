use cms::config::Config;
use cms::database::init_db_with_config;
use cms::grpc::server::spawn_grpc_server;
use cms::repository::Repository;
use cms::router::create_router;
use cms::services::Services;
use cms::storage::{self, STORAGE_KIND_FILESYSTEM, STORAGE_KIND_S3, StorageRegistry};

use axum::Router;
use axum::response::Redirect;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let _guard = cms::tracing::init_tracing();

    // Install the default rustls crypto provider (aws-lc-rs).
    // Required when both aws-lc-rs and ring features are enabled transitively.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider — check dependency features");

    let config = Config::from_env();

    let pool = init_db_with_config(&config)
        .await
        .expect("Failed to initialize database");

    let repository = Repository::new(&pool);

    seed_admin(&repository).await;

    let mut storage_registry = StorageRegistry::new();

    if let Some(ref fs_path) = config.storage_fs_path {
        match storage::FileSystemStorage::new(fs_path) {
            Ok(fs) => {
                storage_registry.register(STORAGE_KIND_FILESYSTEM, Arc::new(fs));
                info!("Filesystem storage initialized at {}", fs_path);
            }
            Err(e) => warn!("Failed to init filesystem storage: {}", e),
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
            Err(e) => warn!("Failed to init S3 storage: {}", e),
        }
    }

    if storage_registry.get(STORAGE_KIND_FILESYSTEM).is_none() && storage_registry.get(STORAGE_KIND_S3).is_none() {
        warn!("No storage providers configured. Set STORAGE_FS_PATH or S3_* env vars.");
    }

    let storage_registry = Arc::new(storage_registry);
    let services = Services::new(Arc::new(repository.clone()), &config);
    let app = create_router(
        repository.clone(),
        config.clone(),
        storage_registry.clone(),
        services.clone(),
    );

    // Shared graceful shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        shutdown_signal().await;
        info!("Shutdown signal received, stopping all listeners...");
        let _ = shutdown_tx.send(true);
    });

    let mut set = JoinSet::new();

    // HTTP listener
    let http_addr: SocketAddr = config.bind_address.parse().expect("Invalid BIND_ADDRESS");

    if config.http_disabled {
        info!("HTTP listener disabled by HTTP_DISABLED=true");
    } else if config.http_redirect_to_https {
        info!("HTTP redirect to HTTPS enabled on http://{}", http_addr);
        set.spawn(serve_http_redirect(http_addr, shutdown_rx.clone()));
    } else {
        info!("REST API server running on http://{}", http_addr);
        set.spawn(serve_http(app.clone(), http_addr, shutdown_rx.clone()));
    }

    // HTTPS listener
    if config.tls_enabled {
        if let Some(ref tls_bind) = config.tls_bind_address {
            let tls_addr: SocketAddr = tls_bind.parse().expect("Invalid TLS_BIND_ADDRESS");
            info!("HTTPS server running on https://{}", tls_addr);

            set.spawn(serve_https(app.clone(), tls_addr, config.clone(), shutdown_rx.clone()));
        } else {
            warn!("TLS enabled but TLS_BIND_ADDRESS not set — skipping HTTPS listener");
        }
    }

    // gRPC listener
    let grpc_addr: SocketAddr = config.grpc_bind_address.parse().expect("Invalid GRPC_BIND_ADDRESS");
    {
        let repository = repository.clone();
        let config = config.clone();
        let storage_registry = storage_registry.clone();

        if config.tls_enabled {
            info!("gRPC TLS server listening on {}", grpc_addr);
        } else {
            info!("gRPC server listening on {}", grpc_addr);
        }

        set.spawn(async move {
            let result = spawn_grpc_server(repository, config, storage_registry, grpc_addr, shutdown_rx.clone()).await;
            if let Err(e) = result {
                tracing::error!("gRPC server error: {}", e);
            }
        });
    }

    if set.is_empty() {
        warn!("No listeners configured! Set BIND_ADDRESS or enable TLS with TLS_BIND_ADDRESS.");
        return;
    }

    // Wait for any listener to finish (panic or graceful shutdown)
    while let Some(result) = set.join_next().await {
        match result {
            Ok(()) => info!("A server finished normally"),
            Err(e) if e.is_panic() => {
                tracing::error!("A server panicked: {}", e);
                std::process::abort();
            }
            Err(_) => {} // cancelled (JoinSet shutdown) — normal exit
        }
    }

    info!("All servers stopped, exiting.");
}

async fn serve_http(app: Router, addr: SocketAddr, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind HTTP address");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_rx.changed().await.ok();
        })
        .await
        .expect("HTTP server error");
}

async fn serve_https(
    app: Router,
    addr: SocketAddr,
    config: Config,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    let tls_config = cms::tls::load_axum_tls_config(&config)
        .await
        .expect("Failed to load TLS configuration");

    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();

    tokio::spawn(async move {
        shutdown_rx.changed().await.ok();
        tracing::info!("HTTPS server graceful shutdown initiated...");
        shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(30)));
    });

    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .expect("HTTPS server error");
}

async fn serve_http_redirect(addr: SocketAddr, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
    let redirect_app = Router::new().fallback(redirect_to_https);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind HTTP redirect address");

    axum::serve(listener, redirect_app.into_make_service())
        .with_graceful_shutdown(async move {
            shutdown_rx.changed().await.ok();
        })
        .await
        .expect("HTTP redirect server error");
}

async fn redirect_to_https(req: http::Request<axum::body::Body>) -> Redirect {
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let path = req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    Redirect::permanent(&format!("https://{}{}", host, path))
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

        warn!("Seeded default admin user (admin/admin) — CHANGE THE PASSWORD IMMEDIATELY!");
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
