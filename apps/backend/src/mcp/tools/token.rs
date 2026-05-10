use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{map_err, ok_result, text_result};
use crate::middleware::auth::{Principal, SCOPE_TOKENS_READ, SCOPE_TOKENS_WRITE};
use crate::services::{Services, scope::ScopeChecker};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListInstanceTokensParams;

pub async fn list_instance_tokens(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    _params: Parameters<ListInstanceTokensParams>,
) -> Result<CallToolResult, McpError> {
    if !principal.is_instance_token() {
        return Err(McpError::invalid_params("Instance token required", None));
    }
    scope.check_scope(principal, SCOPE_TOKENS_READ).map_err(map_err)?;
    let tokens = services.access_token.list_instance_tokens().await.map_err(map_err)?;
    ok_result(&tokens)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateInstanceTokenParams {
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub async fn create_instance_token(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<CreateInstanceTokenParams>,
) -> Result<CallToolResult, McpError> {
    if !principal.is_instance_token() {
        return Err(McpError::invalid_params("Instance token required", None));
    }
    scope.check_scope(principal, SCOPE_TOKENS_WRITE).map_err(map_err)?;
    let token = services
        .access_token
        .create_instance_token(params.0.name.clone(), params.0.scopes.clone())
        .await
        .map_err(map_err)?;
    ok_result(&token)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteInstanceTokenParams {
    pub token_id: String,
}

pub async fn delete_instance_token(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteInstanceTokenParams>,
) -> Result<CallToolResult, McpError> {
    if !principal.is_instance_token() {
        return Err(McpError::invalid_params("Instance token required", None));
    }
    scope.check_scope(principal, SCOPE_TOKENS_WRITE).map_err(map_err)?;
    services
        .access_token
        .delete_instance_token(&params.0.token_id)
        .await
        .map_err(map_err)?;
    Ok(text_result("Instance token deleted"))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSiteTokensParams {
    #[serde(default)]
    pub site_id: Option<String>,
}

pub async fn list_site_tokens(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<ListSiteTokensParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_TOKENS_READ, "admin")
        .await
        .map_err(map_err)?;
    let tokens = services
        .access_token
        .list_site_tokens(&site.site_id)
        .await
        .map_err(map_err)?;
    ok_result(&tokens)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSiteTokenParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub async fn create_site_token(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<CreateSiteTokenParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_TOKENS_WRITE, "admin")
        .await
        .map_err(map_err)?;
    let created_by = principal.user_id();
    let token = services
        .access_token
        .create_site_token(
            &site.site_id,
            params.0.name.clone(),
            params.0.scopes.clone(),
            created_by,
        )
        .await
        .map_err(map_err)?;
    ok_result(&token)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteSiteTokenParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub token_id: String,
}

pub async fn delete_site_token(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteSiteTokenParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_TOKENS_WRITE, "admin")
        .await
        .map_err(map_err)?;
    services
        .access_token
        .delete_site_token(&params.0.token_id, &site.site_id)
        .await
        .map_err(map_err)?;
    Ok(text_result("Site token deleted"))
}
