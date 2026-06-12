use rmcp::{ServiceExt, transport};

use crate::mcp::server::CmsServer;

pub async fn serve(server: CmsServer) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("MCP stdio transport active");
    let service = server.serve(transport::stdio()).await?;

    match service.waiting().await {
        Ok(reason) => {
            tracing::info!(?reason, "MCP stdio transport stopped");
            Ok(())
        }
        Err(error) => {
            tracing::error!(error = %error, "MCP stdio transport failed");
            Err(error.into())
        }
    }
}
