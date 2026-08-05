use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{ok_result, tool_error};
use crate::middleware::auth::Actor;
use crate::models::authorization::Action;
use crate::services::{Services, authorization::AuthorizationService};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSiteParams {
    pub site_id: String,
}

pub async fn get_site(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<GetSiteParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id;
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::SiteRead)
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
    pub site_id: String,
    pub name: Option<String>,
}

pub async fn update_site(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateSiteParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::SiteManage)
        .await
    {
        return Ok(tool_error(e));
    }
    match services.site.update_site(&site_id, params.0.name.as_deref()).await {
        Ok(site) => ok_result(&site),
        Err(e) => Ok(tool_error(e)),
    }
}
