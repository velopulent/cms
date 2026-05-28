pub mod auth;
pub mod client;
pub mod fixtures;

use std::sync::Arc;

use cms::config::Config;
use cms::database::init_db_with_config;
use cms::repository::Repository;
use cms::router::create_router;
use cms::services::Services;
use cms::storage::{StorageRegistry, STORAGE_KIND_FILESYSTEM};
use tokio::net::TcpListener;

pub struct TestServer {
    pub base_url: String,
    _shutdown: tokio::sync::oneshot::Sender<()>,
    _storage_dir: tempfile::TempDir,
}

impl TestServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind random port");
        let port = listener.local_addr().unwrap().port();

        let storage_dir = tempfile::tempdir().expect("Failed to create temp storage dir");
        let storage_path = storage_dir.path().to_str().unwrap().to_string();

        let mut config = Config::default();
        config.database_url = "sqlite::memory:".to_string();
        config.jwt_secret = "test-jwt-secret-integration".to_string();
        config.hmac_secret = "test-hmac-secret-integration".to_string();
        config.storage_fs_path = Some(storage_path.clone());
        config.cookie_secure = false;
        config.mcp_enabled = false;
        config.rate_limit_max_requests = 10000;
        config.rate_limit_window_secs = 60;
        config.db_max_connections = 5;
        config.db_min_connections = 1;
        config.db_acquire_timeout_secs = 30;
        config.db_idle_timeout_secs = 600;
        config.max_upload_size_bytes = 50 * 1024 * 1024;

        let pool = init_db_with_config(&config)
            .await
            .expect("Failed to initialize test database");

        let repository = Repository::new(&pool);

        seed_admin(&repository).await;

        let mut storage_registry = StorageRegistry::new();
        let fs_storage = cms::storage::FileSystemStorage::new(&storage_path)
            .expect("Failed to init filesystem storage");
        storage_registry.register(STORAGE_KIND_FILESYSTEM, Arc::new(fs_storage));
        let storage_registry = Arc::new(storage_registry);

        let services = Services::new(Arc::new(repository.clone()), &config);

        let app = create_router(repository.clone(), config.clone(), storage_registry, services);

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            if let Err(e) = server.await {
                eprintln!("Test server error: {}", e);
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        TestServer {
            base_url: format!("http://127.0.0.1:{}", port),
            _shutdown: shutdown_tx,
            _storage_dir: storage_dir,
        }
    }

    pub async fn register_user(
        &self,
        client: &reqwest::Client,
        username: &str,
        email: &str,
        password: &str,
    ) -> reqwest::Response {
        let resp = client
            .post(format!("{}/api/auth/register", self.base_url))
            .json(&serde_json::json!({
                "username": username,
                "email": email,
                "password": password,
            }))
            .send()
            .await
            .expect("Failed to send register request");

        assert!(
            resp.status().is_success(),
            "Register failed: {} {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );

        resp
    }

    pub async fn register_user_expect_error(
        &self,
        client: &reqwest::Client,
        username: &str,
        email: &str,
        password: &str,
    ) -> reqwest::Response {
        client
            .post(format!("{}/api/auth/register", self.base_url))
            .json(&serde_json::json!({
                "username": username,
                "email": email,
                "password": password,
            }))
            .send()
            .await
            .expect("Failed to send register request")
    }

    pub async fn login_user(
        &self,
        client: &reqwest::Client,
        username: &str,
        password: &str,
    ) -> reqwest::Response {
        client
            .post(format!("{}/api/auth/login", self.base_url))
            .json(&serde_json::json!({
                "username": username,
                "password": password,
            }))
            .send()
            .await
            .expect("Failed to send login request")
    }

    pub async fn me(&self, client: &reqwest::Client) -> reqwest::Response {
        client
            .get(format!("{}/api/auth/me", self.base_url))
            .send()
            .await
            .expect("Failed to send me request")
    }
}

async fn seed_admin(repository: &Repository) {
    if !repository.user.exists("admin").await.unwrap_or(false) {
        let id = uuid::Uuid::now_v7().to_string();
        let password_hash =
            bcrypt::hash("admin", bcrypt::DEFAULT_COST).expect("Failed to hash password");
        repository
            .user
            .create(&id, "admin", "admin@cms.local", &password_hash)
            .await
            .expect("Failed to seed admin user");
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {}
}
