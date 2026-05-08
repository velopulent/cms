use std::sync::Arc;

use tracing::info;

use crate::config::Config;
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::StorageRegistry;
use crate::mcp::server::CmsServer;

pub async fn run_stdio_server(
    services: Arc<Services>,
    repository: Arc<Repository>,
    storage_registry: Arc<StorageRegistry>,
    config: Arc<Config>,
) {
    info!("Starting MCP stdio server");

    let server = CmsServer::new(services, repository, storage_registry, config);

    use rmcp::ServiceExt;
    use rmcp::transport::stdio;

    let service = match server.serve(stdio()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to start MCP stdio server: {}", e);
            return;
        }
    };

    if let Err(e) = service.waiting().await {
        tracing::error!("MCP stdio server error: {}", e);
    }
}