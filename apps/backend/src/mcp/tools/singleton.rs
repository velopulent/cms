use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{ok_result, tool_error};
use crate::mcp::schema::ArbitraryJson;
use crate::middleware::auth::{Actor, Scope};
use crate::services::{Services, scope::ScopeChecker};
use crate::storage::StorageRegistry;

fn require_site_id(actor: &Actor) -> Result<String, McpError> {
    actor
        .bound_site_id()
        .map(String::from)
        .ok_or_else(|| McpError::invalid_request("No site context", None))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSingletonsParams {}

pub async fn list_singletons(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    _params: Parameters<ListSingletonsParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::EntriesRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }
    match services.singleton.list_singletons(&site_id).await {
        Ok(singletons) => ok_result(&singletons),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSingletonParams {
    pub slug: String,
}

pub async fn get_singleton(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    storage_registry: &Arc<StorageRegistry>,
    actor: &Actor,
    params: Parameters<GetSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::EntriesRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }

    let storage_provider = services.file.get_storage_provider(&site_id).await;
    let storage_provider = match storage_provider {
        Ok(p) => p,
        Err(e) => return Ok(tool_error(e)),
    };
    let storage = match storage_registry.get(&storage_provider) {
        Some(s) => s,
        None => return Ok(tool_error(crate::services::error::ServiceError::Internal("Storage not configured".into()))),
    };

    match services
        .singleton
        .get_singleton(&site_id, &params.0.slug, storage)
        .await
    {
        Ok(singleton) => ok_result(&singleton),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSingletonParams {
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
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::EntriesWrite, "editor")
        .await
    {
        return Ok(tool_error(e));
    }

    match services
        .singleton
        .update_singleton(&site_id, &params.0.slug, &params.0.data)
        .await
    {
        Ok(singleton) => ok_result(&singleton),
        Err(e) => Ok(tool_error(e)),
    }
}
