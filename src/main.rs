mod config;
mod database;
mod graphql;
mod middleware {
    pub mod auth;
}
mod models {
    pub mod api_key;
    pub mod collection;
    pub mod content;
    pub mod file;
    pub mod site;
    pub mod user;
}
mod handlers {
    pub mod api_key_handler;
    pub mod auth_handler;
    pub mod collection_handler;
    pub mod content_handler;
    pub mod file_handler;
    pub mod singleton_handler;
    pub mod site_handler;
    pub mod ui_handler;
}
mod repository;
mod router;
mod storage;

use std::net::SocketAddr;

use bcrypt::{DEFAULT_COST, hash};
use uuid::Uuid;

use config::Config;
use database::init_db;
use handlers::file_handler::StorageManager;
use repository::Repository;
use router::create_router;

#[tokio::main]
async fn main() {
    let config = Config::from_env();

    let pool = init_db(&config.database_url).await.expect("Failed to initialize database");

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
                println!("Filesystem storage initialized at {}", fs_path);
            }
            Err(e) => eprintln!("Failed to init filesystem storage: {}", e),
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
                println!("S3 storage initialized");
            }
            Err(e) => eprintln!("Failed to init S3 storage: {}", e),
        }
    }

    if !storage_manager.has_any() {
        eprintln!(
            "WARNING: No storage providers configured. Set STORAGE_FS_PATH or S3_* env vars."
        );
    }

    let app = create_router(repository, config.clone(), storage_manager);

    let addr: SocketAddr = config.bind_address.parse().expect("Invalid BIND_ADDRESS");
    println!("Server running on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn seed_admin(repository: &Repository) {
    if !repository.user.exists("admin").await.unwrap_or(false) {
        let id = Uuid::now_v7().to_string();
        let password_hash = hash("admin", DEFAULT_COST).expect("Failed to hash password");
        repository.user.create(&id, "admin", "admin@cms.local", &password_hash)
            .await
            .expect("Failed to seed admin user");

        println!("Seeded default admin user (admin/admin)");
    }
}
