use std::sync::Arc;

use crate::config::Config;
use crate::repository::Repository;
use crate::storage::StorageRegistry;
use axum::Router;
use tokio_util::sync::CancellationToken;

pub fn mcp_routes(
    repository: Arc<Repository>,
    config: Arc<Config>,
    storage_registry: Arc<StorageRegistry>,
    cancellation_token: CancellationToken,
) -> Router {
    let services = Arc::new(crate::services::Services::new((*repository).clone(), &config));

    crate::mcp::transports::http::mcp_router(services, repository, storage_registry, config, cancellation_token)
}
