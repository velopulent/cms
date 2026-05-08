use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, StreamableHttpServerConfig,
    session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::StorageRegistry;
use crate::mcp::server::CmsServer;

pub fn mcp_router(
    services: Arc<Services>,
    repository: Arc<Repository>,
    storage_registry: Arc<StorageRegistry>,
    config: Arc<Config>,
    cancellation_token: CancellationToken,
) -> Router {
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(CmsServer::new(
                services.clone(),
                repository.clone(),
                storage_registry.clone(),
                config.clone(),
            ))
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_cancellation_token(cancellation_token),
    );

    Router::new().nest_service("/mcp", mcp_service)
}