use std::sync::Arc;

use rmcp::model::CallToolResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Content;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::middleware::auth::{Principal, SCOPE_SCHEMA_READ, SCOPE_SCHEMA_WRITE};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};
use crate::storage::StorageRegistry;

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data).unwrap_or_default();
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_err(e: impl Into<ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSingletonsParams {
    pub site_id: String,
}

pub async fn list_singletons(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<ListSingletonsParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_READ, "viewer").await.map_err(map_err)?;
    let singletons = services.singleton.list_singletons(&params.0.site_id).await.map_err(map_err)?;
    ok_result(&singletons)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSingletonParams {
    pub site_id: String,
    pub slug: String,
}

pub async fn get_singleton(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    storage_registry: &Arc<StorageRegistry>,
    principal: &Principal,
    params: Parameters<GetSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_READ, "viewer")
        .await.map_err(map_err)?;

    let storage_provider = services.file.get_storage_provider(&site.site_id).await.map_err(map_err)?;
    let storage = storage_registry.get(&storage_provider)
        .ok_or_else(|| McpError::internal_error("Storage not configured", None))?;

    let singleton = services.singleton.get_singleton(&site.site_id, &params.0.slug, storage).await.map_err(map_err)?;
    ok_result(&singleton)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSingletonParams {
    pub site_id: String,
    pub slug: String,
    pub data: serde_json::Value,
}

pub async fn update_singleton(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<UpdateSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_WRITE, "editor")
        .await.map_err(map_err)?;

    let singleton = services.singleton.update_singleton(&site.site_id, &params.0.slug, &params.0.data)
        .await.map_err(map_err)?;
    ok_result(&singleton)
}