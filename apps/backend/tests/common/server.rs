//! `TestServer`: a real Axum server (REST + GraphQL + MCP) bound to a random
//! port, backed by an in-memory SQLite DB and temp storage, seeded with an
//! `instance_owner` admin.

use std::sync::Arc;
use std::time::Duration;

use cms::config::Config;
use cms::database::init_db_with_config;
use cms::repository::Repository;
use cms::router::create_router;
use cms::services::Services;
use cms::storage::{STORAGE_KIND_FILESYSTEM, StorageRegistry};
use tokio::net::TcpListener;

pub struct TestServer {
    pub base_url: String,
    _shutdown: tokio::sync::oneshot::Sender<()>,
    _storage_dir: tempfile::TempDir,
    // Dropped last: best-effort drops the per-test Postgres/MySQL database
    // (no-op for the default SQLite `:memory:` backend).
    _db: super::test_db::TestDbHandle,
}

impl TestServer {
    /// Start a server with full-text search disabled (entry search uses SQL `LIKE`).
    pub async fn start() -> Self {
        Self::start_inner(false).await
    }

    /// Start a server with the Tantivy full-text search index enabled, isolated to
    /// this server's temp directory.
    pub async fn start_with_search() -> Self {
        Self::start_inner(true).await
    }

    async fn start_inner(search_enabled: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind random port");
        let addr = listener.local_addr().unwrap();
        let port = addr.port();

        let storage_dir = tempfile::tempdir().expect("Failed to create temp storage dir");
        let storage_path = storage_dir.path().to_str().unwrap().to_string();

        // Provision an isolated database for this server: `sqlite::memory:` by
        // default, or a fresh `cms_test_<id>` Postgres/MySQL database when
        // `TEST_DATABASE` selects one. The handle drops the database on teardown.
        let (database_url, db_handle) = super::test_db::provision().await;

        let config = Config {
            database_url,
            hmac_secret: "test-hmac-secret-integration".to_string(),
            storage_fs_path: Some(storage_path.clone()),
            cookie_secure: false,
            // Real config defaults this to 24h; `Config::default()` leaves it 0, which
            // mints already-expired sessions. SQLite hid that (it compares the datetime
            // columns lexicographically, where the stored `…T…+00:00` sorts above
            // `datetime('now')`); Postgres/MySQL do a real timestamp compare and reject.
            session_lifetime_hours: 24,
            mcp_enabled: true,
            mcp_allowed_hosts: vec!["127.0.0.1".to_string()],
            mcp_allowed_origins: vec![],
            rate_limit_max_requests: 10000,
            rate_limit_window_secs: 60,
            // Tests spin up many servers in parallel against one shared Postgres/MySQL
            // instance. Keep each pool tiny and release idle connections fast so the
            // aggregate stays well under the server's connection ceiling (Postgres
            // defaults to 100); otherwise high-core CI runners intermittently exhaust
            // it. A single TestServer needs almost no internal concurrency.
            db_max_connections: 2,
            db_min_connections: 1,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 5,
            max_upload_size_bytes: 50 * 1024 * 1024,
            // `Config::default()` leaves this 0 => tokens minted already expired.
            upload_token_expiry_secs: 900,
            public_registration_enabled: true,
            bcrypt_cost: bcrypt::DEFAULT_COST,
            webhook_allow_private_targets: true,
            backup_local_path: Some(storage_dir.path().join("backups").to_string_lossy().into_owned()),
            // Deterministic 32-byte (hex) key so encrypted-backup tests can round-trip.
            backup_encryption_key: Some("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff".to_string()),
            backup_enabled: false, // don't run the poller during tests
            search_enabled,
            search_index_path: Some(storage_dir.path().join("search").to_string_lossy().into_owned()),
            ..Default::default()
        };

        let pool = init_db_with_config(&config)
            .await
            .expect("Failed to initialize test database");

        let repository = Repository::new(&pool);

        seed_admin(&repository).await;

        let mut storage_registry = StorageRegistry::new();
        let fs_storage =
            cms::storage::FileSystemStorage::new(&storage_path).expect("Failed to init filesystem storage");
        storage_registry.register(STORAGE_KIND_FILESYSTEM, Arc::new(fs_storage));
        let storage_registry = Arc::new(storage_registry);

        let services = Services::new(Arc::new(repository.clone()), &pool, &config);

        // In search-enabled tests, run the indexer (single consumer) so enqueued
        // writes get applied to the index — mirroring `cms serve`.
        if let (Some(search), Some(queue)) = (services.search.clone(), services.search_queue.clone()) {
            let repo = Arc::new(repository.clone());
            tokio::spawn(async move {
                if search.is_empty() {
                    let _ = search.rebuild_all(&repo).await;
                }
                cms::services::search::indexer::run(search, queue, repo).await;
            });
        }

        let backup_destination =
            cms::services::backup::build_backup_destination(&config).expect("Failed to init backup destination");
        let backup_service = Arc::new(cms::services::backup::BackupService::new(
            pool.clone(),
            storage_registry.clone(),
            backup_destination,
            &config,
        ));

        let app = create_router(
            pool.clone(),
            repository.clone(),
            config.clone(),
            storage_registry,
            services,
            backup_service,
        );

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            let server = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            if let Err(e) = server.await {
                eprintln!("Test server error: {}", e);
            }
        });

        super::wait_for_tcp(addr, Duration::from_secs(5)).await;

        TestServer {
            base_url: format!("http://127.0.0.1:{}", port),
            _shutdown: shutdown_tx,
            _storage_dir: storage_dir,
            _db: db_handle,
        }
    }

    pub async fn login_user(&self, client: &reqwest::Client, email: &str, password: &str) -> reqwest::Response {
        client
            .post(format!("{}/api/auth/login", self.base_url))
            .json(&serde_json::json!({
                "email": email,
                "password": password,
            }))
            .send()
            .await
            .expect("Failed to send login request")
    }
}

pub(crate) async fn seed_admin(repository: &Repository) {
    if !repository.user.exists("admin@cms.local").await.unwrap_or(false) {
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
            .expect("Failed to promote test admin");
    }
}
