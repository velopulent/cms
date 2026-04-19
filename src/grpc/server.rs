use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use tonic::transport::Server;
use tracing::info;

use crate::config::Config;
use crate::grpc::middleware::AuthLayer;
use crate::grpc::services::admin_membership::MembershipServiceImpl;
use crate::grpc::services::admin_site::SiteServiceImpl;
use crate::grpc::services::admin_token::AdminTokenServiceImpl;
use crate::grpc::services::collection::CollectionServiceImpl;
use crate::grpc::services::entry::EntryServiceImpl;
use crate::grpc::services::file::FileServiceImpl;
use crate::grpc::services::singleton::SingletonServiceImpl;
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::StorageRegistry;

pub async fn start_grpc_server(
    repository: Repository,
    config: Config,
    storage_registry: Arc<StorageRegistry>,
    grpc_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let repository = Arc::new(repository);
    let config = Arc::new(config);

    let services = Services::new((*repository).clone(), &config);

    let auth_layer = AuthLayer::new(repository.clone(), config.clone());

    let collection_svc = CollectionServiceImpl::new(services.collection.clone());
    let entry_svc = EntryServiceImpl::new(services.entry.clone());
    let singleton_svc = SingletonServiceImpl::new(services.singleton.clone(), storage_registry);
    let file_svc = FileServiceImpl::new(services.file.clone());
    let site_svc = SiteServiceImpl::new(services.site.clone());
    let membership_svc = MembershipServiceImpl::new(services.site.clone());
    let token_svc = AdminTokenServiceImpl::new(services.access_token.clone());

    let collection_svc = crate::grpc::cms::v1::collection_service_server::CollectionServiceServer::new(collection_svc);
    let entry_svc = crate::grpc::cms::v1::entry_service_server::EntryServiceServer::new(entry_svc);
    let singleton_svc = crate::grpc::cms::v1::singleton_service_server::SingletonServiceServer::new(singleton_svc);
    let file_svc = crate::grpc::cms::v1::file_service_server::FileServiceServer::new(file_svc);
    let site_svc = crate::grpc::cms::v1::site_service_server::SiteServiceServer::new(site_svc);
    let membership_svc = crate::grpc::cms::v1::membership_service_server::MembershipServiceServer::new(membership_svc);
    let token_svc = crate::grpc::cms::v1::token_service_server::TokenServiceServer::new(token_svc);

    info!("gRPC server listening on {}", grpc_addr);

    Server::builder()
        .layer(auth_layer)
        .add_service(collection_svc)
        .add_service(entry_svc)
        .add_service(singleton_svc)
        .add_service(file_svc)
        .add_service(site_svc)
        .add_service(membership_svc)
        .add_service(token_svc)
        .serve(grpc_addr)
        .await?;

    Ok(())
}

pub fn spawn_grpc_server(
    repository: Repository,
    config: Config,
    storage_registry: Arc<StorageRegistry>,
    grpc_addr: SocketAddr,
) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>> {
    Box::pin(start_grpc_server(repository, config, storage_registry, grpc_addr))
}
