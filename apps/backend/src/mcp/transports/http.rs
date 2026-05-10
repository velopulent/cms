use std::sync::Arc;

use axum::{Extension, Router, middleware};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::mcp::server::CmsServer;
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::StorageRegistry;

pub fn mcp_router(
    services: Arc<Services>,
    repository: Arc<Repository>,
    storage_registry: Arc<StorageRegistry>,
    config: Arc<Config>,
    cancellation_token: CancellationToken,
) -> Router {
    let server_config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(config.mcp_allowed_hosts.clone())
        .with_allowed_origins(config.mcp_allowed_origins.clone())
        .with_cancellation_token(cancellation_token);

    let service_repository = repository.clone();
    let service_config = config.clone();

    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(CmsServer::new(
                services.clone(),
                service_repository.clone(),
                storage_registry.clone(),
                service_config.clone(),
            ))
        },
        LocalSessionManager::default().into(),
        server_config,
    );

    Router::new()
        .nest_service("/mcp", mcp_service)
        .layer(middleware::from_fn(crate::mcp::auth::authenticate_mcp_request))
        .layer(Extension((*repository).clone()))
        .layer(Extension((*config).clone()))
}
