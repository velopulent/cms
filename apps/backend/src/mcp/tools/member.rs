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
    let json = serde_json::to_string_pretty(data).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize response: {}", e), None)
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_err(e: impl Into<ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListMembersParams {
    pub site_id: String,
}

pub async fn list_members(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListMembersParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::SiteRead, "viewer")
        .await
        .map_err(map_err)?;
    match services.site.list_members(&params.0.site_id).await {
        Ok(members) => ok_result(&members),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteMemberParams {
    pub site_id: String,
    pub email: String,
    pub role: String,
}

pub async fn invite_member(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<InviteMemberParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::SiteRead, "admin")
        .await
        .map_err(map_err)?;
    let user_id = actor.user_id().unwrap_or("system");
    match services
        .site
        .invite_member(&params.0.site_id, &params.0.email, &params.0.role, user_id)
        .await
    {
        Ok(member) => ok_result(&member),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveMemberParams {
    pub site_id: String,
    pub user_id: String,
}

pub async fn remove_member(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<RemoveMemberParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::SiteRead, "admin")
        .await
        .map_err(map_err)?;
    match services
        .site
        .remove_member(&params.0.site_id, &params.0.user_id)
        .await
    {
        Ok(true) => ok_result(&serde_json::json!({"deleted": true})),
        Ok(false) => ok_result(&serde_json::json!({"error": "Member not found"})),
        Err(e) => Err(map_err(e)),
    }
}
