use std::sync::Arc;

use crate::config::Config;
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::StorageRegistry;
use axum::Router;
use tokio_util::sync::CancellationToken;

pub fn mcp_routes(
    services: Arc<Services>,
    repository: Arc<Repository>,
    config: Arc<Config>,
    storage_registry: Arc<StorageRegistry>,
    cancellation_token: CancellationToken,
) -> Router {
    crate::mcp::transports::http::mcp_router(services, repository, storage_registry, config, cancellation_token)
}
