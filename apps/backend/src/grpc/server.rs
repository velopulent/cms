use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use crate::config::Config;
use crate::grpc::interceptor::AuthInterceptor;
use crate::grpc::services::admin_site::SiteServiceImpl;
use crate::grpc::services::admin_webhook::WebhookServiceImpl;
use crate::grpc::services::collection::CollectionServiceImpl;
use crate::grpc::services::entry::EntryServiceImpl;
use crate::grpc::services::file::FileServiceImpl;
use crate::grpc::services::singleton::SingletonServiceImpl;
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::StorageRegistry;

/// Boxed, pinned future returned by [`spawn_grpc_server`] (the gRPC server task).
type GrpcServerFuture = Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>>;

pub async fn start_grpc_server(
    services: Services,
    repository: Arc<Repository>,
    config: Arc<Config>,
    storage_registry: Arc<StorageRegistry>,
    listener: tokio::net::TcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let collection_svc = CollectionServiceImpl::new(services.collection.clone(), repository.clone());
    let entry_svc = EntryServiceImpl::new(services.entry.clone(), repository.clone());
    let singleton_svc = SingletonServiceImpl::new(services.singleton.clone(), storage_registry, repository.clone());
    let file_svc = FileServiceImpl::new(services.file.clone(), repository.clone());
    let site_svc = SiteServiceImpl::new(services.site.clone(), repository.clone());
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
    let file_svc =
        crate::grpc::cms::v1::file_service_server::FileServiceServer::with_interceptor(file_svc, interceptor.clone());
    let site_svc =
        crate::grpc::cms::v1::site_service_server::SiteServiceServer::with_interceptor(site_svc, interceptor.clone());
    let webhook_svc =
        crate::grpc::cms::v1::webhook_service_server::WebhookServiceServer::with_interceptor(webhook_svc, interceptor);

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tonic::include_file_descriptor_set!("cms_descriptor"))
        .build_v1()
        .expect("Failed to build reflection service");

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<crate::grpc::cms::v1::collection_service_server::CollectionServiceServer<CollectionServiceImpl>>(
        )
        .await;
    health_reporter
        .set_serving::<crate::grpc::cms::v1::entry_service_server::EntryServiceServer<EntryServiceImpl>>()
        .await;
    health_reporter
        .set_serving::<crate::grpc::cms::v1::singleton_service_server::SingletonServiceServer<SingletonServiceImpl>>()
        .await;
    health_reporter
        .set_serving::<crate::grpc::cms::v1::file_service_server::FileServiceServer<FileServiceImpl>>()
        .await;
    health_reporter
        .set_serving::<crate::grpc::cms::v1::site_service_server::SiteServiceServer<SiteServiceImpl>>()
        .await;
    health_reporter
        .set_serving::<crate::grpc::cms::v1::webhook_service_server::WebhookServiceServer<WebhookServiceImpl>>()
        .await;

    Server::builder()
        .add_service(reflection_service)
        .add_service(health_service)
        .add_service(collection_svc)
        .add_service(entry_svc)
        .add_service(singleton_svc)
        .add_service(file_svc)
        .add_service(site_svc)
        .add_service(webhook_svc)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), shutdown)
        .await?;

    Ok(())
}

pub fn spawn_grpc_server(
    services: Services,
    repository: Arc<Repository>,
    config: Arc<Config>,
    storage_registry: Arc<StorageRegistry>,
    listener: tokio::net::TcpListener,
    shutdown: Pin<Box<dyn Future<Output = ()> + Send>>,
) -> GrpcServerFuture {
    Box::pin(start_grpc_server(
        services,
        repository,
        config,
        storage_registry,
        listener,
        shutdown,
    ))
}
