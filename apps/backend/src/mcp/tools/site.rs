use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{ok_result, tool_error};
use crate::middleware::auth::{Actor, Scope};
use crate::services::{Services, scope::ScopeChecker};

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
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::SiteRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }
    match services.site.get_site(&site_id).await {
        Ok(Some(site)) => ok_result(&site),
        Ok(None) => Ok(tool_error(crate::services::error::ServiceError::NotFound(
            "Site not found".into(),
        ))),
        Err(e) => Ok(tool_error(e)),
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
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::SiteRead, "admin")
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .site
        .update_site(&site_id, params.0.name.as_deref())
        .await
    {
        Ok(site) => ok_result(&site),
        Err(e) => Ok(tool_error(e)),
    }
}
