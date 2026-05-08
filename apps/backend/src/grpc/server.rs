use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use tonic::transport::Server;
use tracing::info;

use crate::config::Config;
use crate::grpc::interceptor::AuthInterceptor;
use crate::grpc::services::admin_membership::MembershipServiceImpl;
use crate::grpc::services::admin_site::SiteServiceImpl;
use crate::grpc::services::admin_token::AdminTokenServiceImpl;
use crate::grpc::services::admin_webhook::WebhookServiceImpl;
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

    let collection_svc = CollectionServiceImpl::new(services.collection.clone(), repository.clone());
    let entry_svc = EntryServiceImpl::new(services.entry.clone(), repository.clone());
    let singleton_svc = SingletonServiceImpl::new(services.singleton.clone(), storage_registry, repository.clone());
    let file_svc = FileServiceImpl::new(services.file.clone(), repository.clone());
    let site_svc = SiteServiceImpl::new(services.site.clone(), repository.clone());
    let membership_svc = MembershipServiceImpl::new(services.site.clone(), repository.clone());
    let token_svc = AdminTokenServiceImpl::new(services.access_token.clone(), repository.clone());
    let webhook_svc = WebhookServiceImpl::new(services.webhook.clone(), repository.clone());

    let interceptor = AuthInterceptor::new(config.clone());

    let collection_svc = crate::grpc::cms::v1::collection_service_server::CollectionServiceServer::with_interceptor(
        collection_svc,
        interceptor.clone(),
    );
    let entry_svc = crate::grpc::cms::v1::entry_service_server::EntryServiceServer::with_interceptor(
        entry_svc,
        interceptor.clone(),
    );
    let singleton_svc = crate::grpc::cms::v1::singleton_service_server::SingletonServiceServer::with_interceptor(
        singleton_svc,
        interceptor.clone(),
    );
    let file_svc = crate::grpc::cms::v1::file_service_server::FileServiceServer::with_interceptor(
        file_svc,
        interceptor.clone(),
    );
    let site_svc = crate::grpc::cms::v1::site_service_server::SiteServiceServer::with_interceptor(
        site_svc,
        interceptor.clone(),
    );
    let membership_svc = crate::grpc::cms::v1::membership_service_server::MembershipServiceServer::with_interceptor(
        membership_svc,
        interceptor.clone(),
    );
    let token_svc = crate::grpc::cms::v1::token_service_server::TokenServiceServer::with_interceptor(
        token_svc,
        interceptor.clone(),
    );
    let webhook_svc = crate::grpc::cms::v1::webhook_service_server::WebhookServiceServer::with_interceptor(
        webhook_svc,
        interceptor,
    );

    info!("gRPC server listening on {}", grpc_addr);

    Server::builder()
        .add_service(collection_svc)
        .add_service(entry_svc)
        .add_service(singleton_svc)
        .add_service(file_svc)
        .add_service(site_svc)
        .add_service(membership_svc)
        .add_service(token_svc)
        .add_service(webhook_svc)
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
