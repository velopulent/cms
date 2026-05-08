use std::sync::Arc;

use rmcp::model::CallToolResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Content;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::middleware::auth::{Principal, SCOPE_SITES_READ, SCOPE_SITES_WRITE, SCOPE_SITES_DELETE};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data).unwrap_or_default();
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_err(e: impl Into<ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSitesParams;

pub async fn list_sites(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    _params: Parameters<ListSitesParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_admin_scope(principal, None, SCOPE_SITES_READ).await.map_err(map_err)?;
    let sites = services.site.list_sites_for_principal(principal).await.map_err(map_err)?;
    ok_result(&sites)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSiteParams {
    pub site_id: String,
}

pub async fn get_site(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<GetSiteParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_admin_scope(principal, Some(&params.0.site_id), SCOPE_SITES_READ).await.map_err(map_err)?;
    match services.site.get_site(&params.0.site_id).await.map_err(map_err)? {
        Some(site) => ok_result(&site),
        None => Ok(CallToolResult::success(vec![Content::text("Site not found")])),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSiteParams {
    pub name: String,
    #[serde(default = "default_storage")]
    pub storage_provider: String,
}

fn default_storage() -> String {
    "filesystem".to_string()
}

pub async fn create_site(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<CreateSiteParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_admin_scope(principal, None, SCOPE_SITES_WRITE).await.map_err(map_err)?;
    let created_by = principal.user_id().unwrap_or("system");
    let site = services.site.create_site(&params.0.name, Some(&params.0.storage_provider), created_by)
        .await.map_err(map_err)?;
    ok_result(&site)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSiteParams {
    pub site_id: String,
    pub name: Option<String>,
}

pub async fn update_site(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<UpdateSiteParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_admin_scope(principal, Some(&params.0.site_id), SCOPE_SITES_WRITE).await.map_err(map_err)?;
    let site = services.site.update_site(&params.0.site_id, params.0.name.as_deref())
        .await.map_err(map_err)?;
    ok_result(&site)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteSiteParams {
    pub site_id: String,
}

pub async fn delete_site(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteSiteParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_admin_scope(principal, Some(&params.0.site_id), SCOPE_SITES_DELETE).await.map_err(map_err)?;
    services.site.delete_site(&params.0.site_id).await.map_err(map_err)?;
    Ok(CallToolResult::success(vec![Content::text("Site deleted")]))
}