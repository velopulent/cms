use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{map_err, ok_result};
use crate::mcp::schema::ArbitraryJson;
use crate::middleware::auth::{Actor, Scope};
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
    actor: &Actor,
    params: Parameters<ListSingletonsParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.as_deref().unwrap_or_else(|| actor.bound_site_id().unwrap_or(""));
    scope
        .require_site_scope(actor, site_id, &Scope::EntriesRead, "viewer")
        .await
        .map_err(map_err)?;
    let singletons = services.singleton.list_singletons(site_id).await.map_err(map_err)?;
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
    actor: &Actor,
    params: Parameters<GetSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.as_deref().unwrap_or_else(|| actor.bound_site_id().unwrap_or(""));
    scope
        .require_site_scope(actor, site_id, &Scope::EntriesRead, "viewer")
        .await
        .map_err(map_err)?;

    let storage_provider = services.file.get_storage_provider(site_id).await.map_err(map_err)?;
    let storage = storage_registry
        .get(&storage_provider)
        .ok_or_else(|| McpError::internal_error("Storage not configured", None))?;

    let singleton = services
        .singleton
        .get_singleton(site_id, &params.0.slug, storage)
        .await
        .map_err(map_err)?;
    ok_result(&singleton)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSingletonParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub slug: String,
    #[schemars(with = "ArbitraryJson")]
    pub data: serde_json::Value,
}

pub async fn update_singleton(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.as_deref().unwrap_or_else(|| actor.bound_site_id().unwrap_or(""));
    scope
        .require_site_scope(actor, site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;

    let singleton = services
        .singleton
        .update_singleton(site_id, &params.0.slug, &params.0.data)
        .await
        .map_err(map_err)?;
    ok_result(&singleton)
}
