use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::middleware::auth::{Principal, SCOPE_MEMBERS_READ, SCOPE_MEMBERS_WRITE};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| McpError::internal_error(format!("Serialization failed: {}", e), None))?;
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
    principal: &Principal,
    params: Parameters<ListMembersParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_admin_scope(principal, Some(&params.0.site_id), SCOPE_MEMBERS_READ)
        .await
        .map_err(map_err)?;
    let members = services.site.list_members(&params.0.site_id).await.map_err(map_err)?;
    ok_result(&members)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteMemberParams {
    pub site_id: String,
    pub username: String,
    pub role: String,
}

pub async fn invite_member(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<InviteMemberParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_admin_scope(principal, Some(&params.0.site_id), SCOPE_MEMBERS_WRITE)
        .await
        .map_err(map_err)?;
    let member = services
        .site
        .invite_member(&params.0.site_id, &params.0.username, &params.0.role)
        .await
        .map_err(map_err)?;
    ok_result(&member)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveMemberParams {
    pub site_id: String,
    pub user_id: String,
}

pub async fn remove_member(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<RemoveMemberParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_admin_scope(principal, Some(&params.0.site_id), SCOPE_MEMBERS_WRITE)
        .await
        .map_err(map_err)?;
    let by_user_id = principal.user_id().unwrap_or("system");
    services
        .site
        .remove_member(&params.0.site_id, &params.0.user_id, by_user_id)
        .await
        .map_err(map_err)?;
    Ok(CallToolResult::success(vec![Content::text("Member removed")]))
}
