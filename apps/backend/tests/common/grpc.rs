//! `GrpcTestContext`: a tonic gRPC server (with the real `AuthInterceptor`)
//! plus a sibling Axum server for REST-based seeding, sharing one DB/storage.

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use cms::config::Config;
use cms::database::init_db_with_config;
use cms::grpc::interceptor::AuthInterceptor;
use cms::grpc::services::admin_site::SiteServiceImpl;
use cms::grpc::services::admin_webhook::WebhookServiceImpl;
use cms::grpc::services::collection::CollectionServiceImpl;
use cms::grpc::services::entry::EntryServiceImpl;
use cms::grpc::services::file::FileServiceImpl;
use cms::grpc::services::singleton::SingletonServiceImpl;
use cms::repository::Repository;
use cms::router::create_router;
use cms::services::Services;
use cms::storage::{STORAGE_KIND_FILESYSTEM, StorageRegistry};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::Request;
use tonic::transport::Channel;

use super::auth::extract_cookies;
use super::client::http_client;
use super::server::seed_admin;

pub struct GrpcTestContext {
    pub grpc_addr: SocketAddr,
    pub rest_base_url: String,
    _shutdown: tokio::sync::oneshot::Sender<()>,
    _grpc_shutdown: tokio::sync::oneshot::Sender<()>,
    indexer: Option<tokio::task::JoinHandle<()>>,
    _storage_dir: tempfile::TempDir,
}

impl Drop for GrpcTestContext {
    fn drop(&mut self) {
        if let Some(indexer) = self.indexer.take() {
            indexer.abort();
        }
    }
}

impl GrpcTestContext {
    pub async fn start() -> Self {
        let storage_dir = tempfile::tempdir().expect("Failed to create temp storage dir");
        let storage_path = storage_dir.path().to_str().unwrap().to_string();

        let config = Config {
            database_url: "sqlite::memory:".to_string(),
            token_index_key: "test-token-index-key".to_string(),
            session_auth_key: "test-session-auth-key".to_string(),
            signed_upload_key: "test-signed-upload-key".to_string(),
            storage_fs_path: Some(storage_path.clone()),
            cookie_secure: false,
            mcp_enabled: false,
            rate_limit_max_requests: 10000,
            rate_limit_window_secs: 60,
            db_max_connections: 5,
            db_min_connections: 1,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            max_upload_size_bytes: 50 * 1024 * 1024,
            public_registration_enabled: true,
            bcrypt_cost: bcrypt::DEFAULT_COST,
            webhook_allow_private_targets: true,
            backup_local_path: Some(format!("{storage_path}/backups")),
            search_index_path: Some(format!("{storage_path}/search")),
            ..Default::default()
        };

        let pool = init_db_with_config(&config)
            .await
            .expect("Failed to initialize test database");

        let repository = Repository::new(&pool);

        seed_admin(&repository).await;

        let storage_registry = StorageRegistry::new();
        let fs_storage =
            cms::storage::FileSystemStorage::new(&storage_path).expect("Failed to init filesystem storage");
        storage_registry.register(STORAGE_KIND_FILESYSTEM, Arc::new(fs_storage));
        let storage_registry = Arc::new(storage_registry);

        let config = Arc::new(config);
        let repository_arc = Arc::new(repository.clone());
        let services = Services::new(repository_arc.clone(), &pool, &config);

        // Mirror server startup: gRPC writes enqueue search updates, so tests need
        // the single index consumer running against an isolated per-test index.
        let indexer = if let (Some(search), Some(queue)) = (services.search.clone(), services.search_queue.clone()) {
            let repo = repository_arc.clone();
            Some(tokio::spawn(async move {
                if search.is_empty() {
                    let _ = search.rebuild_all(&repo).await;
                }
                cms::services::search::indexer::run(search, queue, repo).await;
            }))
        } else {
            None
        };

        // Start Axum server for REST-based seeding
        let axum_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind random port");
        let axum_addr = axum_listener.local_addr().unwrap();
        let rest_base_url = format!("http://127.0.0.1:{}", axum_addr.port());

        let backup_destination =
            cms::services::backup::build_backup_destination(&config).expect("Failed to init backup destination");
        let backup_service = Arc::new(cms::services::backup::BackupService::new(
            pool.clone(),
            storage_registry.clone(),
            backup_destination,
            &config,
        ));
        let settings = cms::services::settings::SettingsService::load(pool.clone(), &"11".repeat(32))
            .await
            .expect("Failed to init instance settings");

        let app = create_router(
            pool.clone(),
            repository.clone(),
            (*config).clone(),
            storage_registry.clone(),
            services.clone(),
            backup_service,
            settings,
        );

        let (axum_shutdown_tx, axum_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::serve(axum_listener, app).with_graceful_shutdown(async move {
                let _ = axum_shutdown_rx.await;
            });
            if let Err(e) = server.await {
                eprintln!("Axum test server error: {}", e);
            }
        });

        // Start tonic gRPC server
        let grpc_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind random port");
        let grpc_addr = grpc_listener.local_addr().unwrap();
        let incoming = TcpListenerStream::new(grpc_listener);

        let collection_svc = CollectionServiceImpl::new(services.collection.clone(), repository_arc.clone());
        let entry_svc = EntryServiceImpl::new(services.entry.clone(), repository_arc.clone());
        let singleton_svc = SingletonServiceImpl::new(
            services.singleton.clone(),
            storage_registry.clone(),
            repository_arc.clone(),
        );
        let file_svc = FileServiceImpl::new(services.file.clone(), repository_arc.clone());
        let site_svc = SiteServiceImpl::new(services.site.clone(), repository_arc.clone());
        let webhook_svc = WebhookServiceImpl::new(services.webhook.clone(), repository_arc);

        let interceptor = AuthInterceptor::new(config.clone());

        let collection_server =
            cms::grpc::cms::v1::collection_service_server::CollectionServiceServer::with_interceptor(
                collection_svc,
                interceptor.clone(),
            );
        let entry_server = cms::grpc::cms::v1::entry_service_server::EntryServiceServer::with_interceptor(
            entry_svc,
            interceptor.clone(),
        );
        let singleton_server = cms::grpc::cms::v1::singleton_service_server::SingletonServiceServer::with_interceptor(
            singleton_svc,
            interceptor.clone(),
        );
        let file_server =
            cms::grpc::cms::v1::file_service_server::FileServiceServer::with_interceptor(file_svc, interceptor.clone());
        let site_server =
            cms::grpc::cms::v1::site_service_server::SiteServiceServer::with_interceptor(site_svc, interceptor.clone());
        let webhook_server = cms::grpc::cms::v1::webhook_service_server::WebhookServiceServer::with_interceptor(
            webhook_svc,
            interceptor,
        );

        let (grpc_shutdown_tx, grpc_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(collection_server)
                .add_service(entry_server)
                .add_service(singleton_server)
                .add_service(file_server)
                .add_service(site_server)
                .add_service(webhook_server)
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = grpc_shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        super::wait_for_tcp(axum_addr, Duration::from_secs(5)).await;
        super::wait_for_tcp(grpc_addr, Duration::from_secs(5)).await;

        GrpcTestContext {
            grpc_addr,
            rest_base_url,
            _shutdown: axum_shutdown_tx,
            _grpc_shutdown: grpc_shutdown_tx,
            indexer,
            _storage_dir: storage_dir,
        }
    }

    pub async fn connect(&self) -> Channel {
        tonic::transport::Channel::from_shared(format!("http://{}", self.grpc_addr))
            .unwrap()
            .connect()
            .await
            .unwrap()
    }

    pub async fn setup_site_and_token(&self) -> (String, String) {
        let client = http_client();

        // Login as admin
        let resp = client
            .post(format!("{}/api/auth/login", self.rest_base_url))
            .json(&serde_json::json!({
                "email": "admin@cms.local",
                "password": "admin",
            }))
            .send()
            .await
            .expect("Failed to login");

        let (token, csrf) = extract_cookies(&resp);

        // Create site
        let resp = client
            .post(format!("{}/api/dashboard/sites", self.rest_base_url))
            .header("Cookie", format!("token={}; csrf={}", token, csrf))
            .header("X-CSRF-Token", &csrf)
            .json(&serde_json::json!({"name": "Test Site", "storage_provider": "filesystem"}))
            .send()
            .await
            .expect("Failed to create site");
        let site: serde_json::Value = resp.json().await.unwrap();
        let site_id = site["id"].as_str().unwrap().to_string();

        // Create access token
        let resp = client
            .post(format!("{}/api/dashboard/sites/{}/tokens", self.rest_base_url, site_id))
            .header("Cookie", format!("token={}; csrf={}", token, csrf))
            .header("X-CSRF-Token", &csrf)
            .json(&serde_json::json!({"name": "Test Token", "permission": "write"}))
            .send()
            .await
            .expect("Failed to create token");
        let token_val: serde_json::Value = resp.json().await.unwrap();
        let token = token_val["token"].as_str().unwrap().to_string();

        (site_id, token)
    }

    pub async fn upload_file(
        &self,
        site_id: &str,
        filename: &str,
        content: &[u8],
        mime_type: &str,
    ) -> serde_json::Value {
        let client = http_client();

        let resp = client
            .post(format!("{}/api/auth/login", self.rest_base_url))
            .json(&serde_json::json!({
                "email": "admin@cms.local",
                "password": "admin",
            }))
            .send()
            .await
            .expect("Failed to login");

        let (token, csrf) = extract_cookies(&resp);

        let part = reqwest::multipart::Part::bytes(content.to_vec())
            .file_name(filename.to_string())
            .mime_str(mime_type)
            .unwrap();

        let form = reqwest::multipart::Form::new()
            .text("site_id", site_id.to_string())
            .part("file", part);

        let resp = client
            .post(format!("{}/api/dashboard/sites/{}/files", self.rest_base_url, site_id))
            .header("Cookie", format!("token={}; csrf={}", token, csrf))
            .header("X-CSRF-Token", &csrf)
            .multipart(form)
            .send()
            .await
            .expect("Failed to upload file");

        resp.json().await.unwrap()
    }
}

pub fn auth_interceptor(token: &str) -> impl tonic::service::Interceptor + Clone + use<> {
    let token = token.to_string();
    move |mut req: Request<()>| {
        req.metadata_mut().insert(
            "authorization",
            tonic::metadata::MetadataValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );
        Ok(req)
    }
}
