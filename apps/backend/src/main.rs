use cms::config::Config;
use cms::database::init_db_with_config;
use cms::grpc::server::spawn_grpc_server;
use cms::repository::Repository;
use cms::router::create_router;
use cms::services::Services;
use cms::storage::{self, STORAGE_KIND_FILESYSTEM, STORAGE_KIND_S3, StorageRegistry};

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::select;
use tracing::{info, warn};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let _guard = cms::tracing::init_tracing();

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
    let services = Services::new(repository.clone(), &config);
    let app = create_router(repository.clone(), config.clone(), storage_registry.clone(), services.clone());

    let addr: SocketAddr = config.bind_address.parse().expect("Invalid BIND_ADDRESS");
    info!("REST API server running on {}", addr);

    let grpc_addr: SocketAddr = config.grpc_bind_address.parse().expect("Invalid GRPC_BIND_ADDRESS");
    info!("gRPC server running on {}", grpc_addr);

    let rest_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind address");

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("Server error");
    });

    let grpc_handle = tokio::spawn(spawn_grpc_server(
        repository.clone(),
        config.clone(),
        storage_registry.clone(),
        grpc_addr,
    ));

    let mcp_stdio_handle = if config.mcp_stdio {
        let mcp_services = Arc::new(services.clone());
        let mcp_repo = Arc::new(repository.clone());
        let mcp_storage = storage_registry.clone();
        let mcp_config = Arc::new(config.clone());
        info!("MCP stdio server enabled");
        Some(tokio::spawn(async move {
            cms::mcp::transports::stdio::run_stdio_server(
                mcp_services, mcp_repo, mcp_storage, mcp_config,
            ).await;
        }))
    } else {
        None
    };

    select! {
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
        result = async {
            if let Some(handle) = mcp_stdio_handle {
                handle.await
            } else {
                std::future::pending().await
            }
        } => {
            if let Err(e) = result {
                tracing::error!("MCP stdio server error: {}", e);
            }
        }
    }
}

async fn seed_admin(repository: &Repository) {
    if !repository.user.exists("admin").await.unwrap_or(false) {
        let id = uuid::Uuid::now_v7().to_string();
        let password_hash = bcrypt::hash("admin", bcrypt::DEFAULT_COST).expect("Failed to hash password");
        repository
            .user
            .create(&id, "admin", "admin@cms.local", &password_hash)
            .await
            .expect("Failed to seed admin user");

        warn!("Seeded default admin user (admin/admin) — CHANGE THE PASSWORD IMMEDIATELY!");
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
