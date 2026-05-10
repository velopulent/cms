use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{map_err, ok_result};
use crate::middleware::auth::{Principal, SCOPE_SCHEMA_READ, SCOPE_SCHEMA_WRITE};
use crate::services::{Services, scope::ScopeChecker};
use crate::storage::StorageRegistry;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSingletonsParams {
    #[serde(default)]
    pub site_id: Option<String>,
}

pub async fn list_singletons(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<ListSingletonsParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_READ, "viewer")
        .await
        .map_err(map_err)?;
    let singletons = services
        .singleton
        .list_singletons(&site.site_id)
        .await
        .map_err(map_err)?;
    ok_result(&singletons)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSingletonParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub slug: String,
}

pub async fn get_singleton(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    storage_registry: &Arc<StorageRegistry>,
    principal: &Principal,
    params: Parameters<GetSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_READ, "viewer")
        .await
        .map_err(map_err)?;

    let storage_provider = services
        .file
        .get_storage_provider(&site.site_id)
        .await
        .map_err(map_err)?;
    let storage = storage_registry
        .get(&storage_provider)
        .ok_or_else(|| McpError::internal_error("Storage not configured", None))?;

    let singleton = services
        .singleton
        .get_singleton(&site.site_id, &params.0.slug, storage)
        .await
        .map_err(map_err)?;
    ok_result(&singleton)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSingletonParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub slug: String,
    pub data: serde_json::Value,
}

pub async fn update_singleton(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<UpdateSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_WRITE, "editor")
        .await
        .map_err(map_err)?;

    let singleton = services
        .singleton
        .update_singleton(&site.site_id, &params.0.slug, &params.0.data)
        .await
        .map_err(map_err)?;
    ok_result(&singleton)
}
