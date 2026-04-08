use cms::config::Config;
use cms::database::init_db_with_config;
use cms::handlers::file_handler::StorageManager;
use cms::repository::Repository;
use cms::router::create_router;
use cms::storage;

use tracing::{info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cms=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();

    let pool = init_db_with_config(&config)
        .await
        .expect("Failed to initialize database");

    let repository = Repository::new(&pool);

    seed_admin(&repository).await;

    let mut storage_manager = StorageManager {
        filesystem: None,
        s3: None,
    };

    if let Some(ref fs_path) = config.storage_fs_path {
        match storage::FileSystemStorage::new(fs_path) {
            Ok(fs) => {
                storage_manager.filesystem = Some(fs);
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
                storage_manager.s3 = Some(s3);
                info!("S3 storage initialized");
            }
            Err(e) => warn!("Failed to init S3 storage: {}", e),
        }
    }

    if !storage_manager.has_any() {
        warn!("No storage providers configured. Set STORAGE_FS_PATH or S3_* env vars.");
    }

    let app = create_router(repository, config.clone(), storage_manager);

    let addr: std::net::SocketAddr = config
        .bind_address
        .parse()
        .expect("Invalid BIND_ADDRESS");
    info!("Server running on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

async fn seed_admin(repository: &Repository) {
    if !repository.user.exists("admin").await.unwrap_or(false) {
        let id = Uuid::now_v7().to_string();
        let password_hash =
            bcrypt::hash("admin", bcrypt::DEFAULT_COST).expect("Failed to hash password");
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
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
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