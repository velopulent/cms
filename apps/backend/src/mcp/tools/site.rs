use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::middleware::auth::{Actor, Scope};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data).map_err(|e| McpError::internal_error(format!("Failed to serialize response: {}", e), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_err(e: impl Into<ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

fn require_site_id(actor: &Actor) -> Result<String, McpError> {
    actor
        .bound_site_id()
        .map(String::from)
        .ok_or_else(|| McpError::invalid_request("No site context", None))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSiteParams {}

pub async fn get_site(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    _params: Parameters<GetSiteParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::SiteRead, "viewer")
        .await
        .map_err(map_err)?;
    match services.site.get_site(&site_id).await.map_err(map_err)? {
        Some(site) => ok_result(&site),
        None => ok_result(&serde_json::json!({"error": "Site not found"})),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSiteParams {
    pub name: Option<String>,
}

pub async fn update_site(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateSiteParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::SiteRead, "admin")
        .await
        .map_err(map_err)?;
    let site = services
        .site
        .update_site(&site_id, params.0.name.as_deref())
        .await
        .map_err(map_err)?;
    ok_result(&site)
}
