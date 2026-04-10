use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use tonic::transport::Server;
use tracing::info;

use crate::config::Config;
use crate::grpc::middleware::AuthLayer;
use crate::grpc::services::api_key::ApiKeyServiceImpl;
use crate::grpc::services::collection::CollectionServiceImpl;
use crate::grpc::services::entry::EntryServiceImpl;
use crate::grpc::services::file::FileServiceImpl;
use crate::grpc::services::singleton::SingletonServiceImpl;
use crate::repository::Repository;

/// Starts the gRPC server with authentication middleware.
///
/// This function initializes the gRPC server with all services and applies
/// the authentication middleware layer to handle API key validation. The
/// middleware performs async database lookups for API key verification.
///
/// # Arguments
/// * `repository` - Database repository for persistence operations
/// * `config` - Application configuration
/// * `grpc_addr` - Socket address to bind the server to
///
/// # Returns
/// * `Ok(())` - Server shut down gracefully
/// * `Err(...)` - Server failed to start or encountered an error
///
/// # Example
/// ```rust,ignore
/// let repo = Repository::new().await?;
/// let config = Config::load()?;
/// start_grpc_server(repo, config, "0.0.0.0:50051".parse()?).await?;
/// ```
pub async fn start_grpc_server(
    repository: Repository,
    config: Config,
    grpc_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repository = Arc::new(repository);
    let config = Arc::new(config);

    // Create the authentication layer
    // This middleware will intercept all requests and validate API keys
    let auth_layer = AuthLayer::new(repository.clone(), config.clone());

    // Initialize service implementations
    let collection_svc = CollectionServiceImpl::new(repository.clone());
    let entry_svc = EntryServiceImpl::new(repository.clone());
    let singleton_svc = SingletonServiceImpl::new(repository.clone());
    let file_svc = FileServiceImpl::new(repository.clone());
    let api_key_svc = ApiKeyServiceImpl::new(repository.clone(), config.hmac_secret.clone());

    // Create tonic service servers
    // Note: We don't use with_interceptor() anymore - authentication is handled
    // by the Tower middleware layer applied at the Server level
    let collection_svc =
        crate::grpc::cms::v1::collection_service_server::CollectionServiceServer::new(
            collection_svc,
        );
    let entry_svc =
        crate::grpc::cms::v1::entry_service_server::EntryServiceServer::new(entry_svc);
    let singleton_svc =
        crate::grpc::cms::v1::singleton_service_server::SingletonServiceServer::new(singleton_svc);
    let file_svc = crate::grpc::cms::v1::file_service_server::FileServiceServer::new(file_svc);
    let api_key_svc =
        crate::grpc::cms::v1::api_key_service_server::ApiKeyServiceServer::new(api_key_svc);

    info!("gRPC server listening on {}", grpc_addr);

    // Build and start the server with authentication middleware
    // The layer() method applies the AuthLayer to all services
    Server::builder()
        .layer(auth_layer)
        .add_service(collection_svc)
        .add_service(entry_svc)
        .add_service(singleton_svc)
        .add_service(file_svc)
        .add_service(api_key_svc)
        .serve(grpc_addr)
        .await?;

    Ok(())
}

/// Spawns the gRPC server as a background task.
///
/// This is a convenience wrapper around `start_grpc_server` that returns
/// a boxed future suitable for spawning with tokio::spawn.
///
/// # Arguments
/// * `repository` - Database repository
/// * `config` - Application configuration
/// * `grpc_addr` - Socket address to bind to
///
/// # Returns
/// A pinned boxed future that resolves when the server shuts down
pub fn spawn_grpc_server(
    repository: Repository,
    config: Config,
    grpc_addr: SocketAddr,
) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>> {
    Box::pin(start_grpc_server(repository, config, grpc_addr))
}
