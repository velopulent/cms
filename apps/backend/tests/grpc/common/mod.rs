use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

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
use cms::services::Services;
use cms::storage::{StorageRegistry, STORAGE_KIND_FILESYSTEM};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Channel;
use tonic::Request;

pub struct GrpcTestContext {
    pub addr: SocketAddr,
    pub repository: Repository,
    pub admin_user_id: String,
    _shutdown: tokio::sync::oneshot::Sender<()>,
    _storage_dir: tempfile::TempDir,
}

impl GrpcTestContext {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind random port");
        let addr = listener.local_addr().unwrap();
        let incoming = TcpListenerStream::new(listener);

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

        let admin_user_id = seed_admin_and_get_id(&repository).await;

        let mut storage_registry = StorageRegistry::new();
        let fs_storage = cms::storage::FileSystemStorage::new(&storage_path)
            .expect("Failed to init filesystem storage");
        storage_registry.register(STORAGE_KIND_FILESYSTEM, Arc::new(fs_storage));
        let storage_registry = Arc::new(storage_registry);

        let config = Arc::new(config);
        let repository_arc = Arc::new(repository.clone());

        let services = Services::new(repository_arc.clone(), &config);

        let collection_svc = CollectionServiceImpl::new(services.collection.clone(), repository_arc.clone());
        let entry_svc = EntryServiceImpl::new(services.entry.clone(), repository_arc.clone());
        let singleton_svc =
            SingletonServiceImpl::new(services.singleton.clone(), storage_registry.clone(), repository_arc.clone());
        let file_svc = FileServiceImpl::new(services.file.clone(), repository_arc.clone());
        let site_svc = SiteServiceImpl::new(services.site.clone(), repository_arc.clone());
        let webhook_svc = WebhookServiceImpl::new(services.webhook.clone(), repository_arc);

        let interceptor = AuthInterceptor::new(config);

        let collection_server =
            cms::grpc::cms::v1::collection_service_server::CollectionServiceServer::with_interceptor(
                collection_svc,
                interceptor.clone(),
            );
        let entry_server = cms::grpc::cms::v1::entry_service_server::EntryServiceServer::with_interceptor(
            entry_svc,
            interceptor.clone(),
        );
        let singleton_server =
            cms::grpc::cms::v1::singleton_service_server::SingletonServiceServer::with_interceptor(
                singleton_svc,
                interceptor.clone(),
            );
        let file_server = cms::grpc::cms::v1::file_service_server::FileServiceServer::with_interceptor(
            file_svc,
            interceptor.clone(),
        );
        let site_server = cms::grpc::cms::v1::site_service_server::SiteServiceServer::with_interceptor(
            site_svc,
            interceptor.clone(),
        );
        let webhook_server = cms::grpc::cms::v1::webhook_service_server::WebhookServiceServer::with_interceptor(
            webhook_svc,
            interceptor,
        );

        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(collection_server)
                .add_service(entry_server)
                .add_service(singleton_server)
                .add_service(file_server)
                .add_service(site_server)
                .add_service(webhook_server)
                .serve_with_incoming(incoming)
                .await
                .unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        GrpcTestContext {
            addr,
            repository,
            admin_user_id,
            _shutdown: shutdown_tx,
            _storage_dir: storage_dir,
        }
    }

    pub async fn connect(&self) -> Channel {
        tonic::transport::Channel::from_shared(format!("http://{}", self.addr))
            .unwrap()
            .connect()
            .await
            .unwrap()
    }
}

#[derive(Clone)]
pub struct TestAuthInterceptor {
    token: String,
}

impl tonic::service::Interceptor for TestAuthInterceptor {
    fn call(&mut self, mut req: Request<()>) -> Result<Request<()>, tonic::Status> {
        req.metadata_mut().insert(
            "authorization",
            tonic::metadata::MetadataValue::from_str(&format!("Bearer {}", self.token)).unwrap(),
        );
        Ok(req)
    }
}

pub fn auth_interceptor(token: &str) -> TestAuthInterceptor {
    TestAuthInterceptor {
        token: token.to_string(),
    }
}

pub async fn seed_access_token(repo: &Repository, site_id: &str, permission: &str) -> String {
    let token = format!("cms_site_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let prefix: String = token.chars().take(24).collect();
    let token_hash = bcrypt::hash(&token, bcrypt::DEFAULT_COST).expect("Failed to hash token");
    let hmac = cms::grpc::interceptor::compute_key_hmac(&token, "test-hmac-secret-integration");
    let id = uuid::Uuid::now_v7().to_string();

    repo.access_token
        .create(
            &id,
            site_id,
            "Test Token",
            &token_hash,
            &prefix,
            &hmac,
            permission,
            None,
        )
        .await
        .expect("Failed to create access token");

    token
}

pub async fn seed_site(repo: &Repository, name: &str, created_by: &str) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    repo.site
        .create(&id, name, "filesystem", created_by)
        .await
        .expect("Failed to seed site");
    id
}

pub async fn seed_admin_and_get_id(repo: &Repository) -> String {
    let username = "admin";
    if let Ok(Some(user)) = repo.user.find_by_username(username).await {
        return user.id;
    }
    let id = uuid::Uuid::now_v7().to_string();
    let password_hash =
        bcrypt::hash("admin", bcrypt::DEFAULT_COST).expect("Failed to hash password");
    repo.user
        .create(&id, username, "admin@cms.local", &password_hash)
        .await
        .expect("Failed to seed admin user");
    id
}
