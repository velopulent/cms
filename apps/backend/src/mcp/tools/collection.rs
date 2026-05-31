use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::schema::ArbitraryJson;
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

fn require_site_id(actor: &Actor) -> Result<String, McpError> {
    actor
        .bound_site_id()
        .map(String::from)
        .ok_or_else(|| McpError::invalid_request("No site context", None))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCollectionsParams {}

pub async fn list_collections(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    _params: Parameters<ListCollectionsParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsRead, "viewer")
        .await
        .map_err(map_err)?;
    match services.collection.list_collections(&site_id).await {
        Ok(collections) => ok_result(&collections),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCollectionParams {
    pub slug: String,
}

pub async fn get_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<GetCollectionParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsRead, "viewer")
        .await
        .map_err(map_err)?;
    match services
        .collection
        .get_collection(&site_id, &params.0.slug)
        .await
    {
        Ok(Some(collection)) => ok_result(&collection),
        Ok(None) => ok_result(&serde_json::json!({"error": "Collection not found"})),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCollectionParams {
    pub name: String,
    pub slug: Option<String>,
    #[schemars(with = "ArbitraryJson")]
    pub definition: serde_json::Value,
    pub is_singleton: Option<bool>,
}

pub async fn create_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<CreateCollectionParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsWrite, "admin")
        .await
        .map_err(map_err)?;
    let slug = params
        .0
        .slug
        .unwrap_or_else(|| params.0.name.to_lowercase().replace(' ', "-"));
    let normalized = crate::services::definition_validation::normalize_definition(&params.0.definition)
        .map_err(|e| McpError::invalid_request(e, None))?;
    let definition = normalized.to_string();
    let is_singleton = params.0.is_singleton.unwrap_or(false);
    match services
        .collection
        .create_collection(&site_id, &params.0.name, &slug, &definition, is_singleton)
        .await
    {
        Ok(collection) => ok_result(&collection),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateCollectionParams {
    pub slug: String,
    pub name: Option<String>,
    pub new_slug: Option<String>,
    #[schemars(with = "ArbitraryJson")]
    pub definition: Option<serde_json::Value>,
}

pub async fn update_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateCollectionParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsWrite, "admin")
        .await
        .map_err(map_err)?;
    let definition_str = match params.0.definition {
        Some(d) => {
            let normalized = crate::services::definition_validation::normalize_definition(&d)
                .map_err(|e| McpError::invalid_request(e, None))?;
            Some(normalized.to_string())
        }
        None => None,
    };
    match services
        .collection
        .update_collection(
            &site_id,
            &params.0.slug,
            params.0.name.as_deref(),
            params.0.new_slug.as_deref(),
            definition_str.as_deref(),
        )
        .await
    {
        Ok(collection) => ok_result(&collection),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteCollectionParams {
    pub slug: String,
}

pub async fn delete_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteCollectionParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsWrite, "admin")
        .await
        .map_err(map_err)?;
    match services
        .collection
        .delete_collection(&site_id, &params.0.slug)
        .await
    {
        Ok(n) => {
            if n > 0 {
                ok_result(&serde_json::json!({"deleted": true}))
            } else {
                ok_result(&serde_json::json!({"error": "Collection not found"}))
            }
        }
        Err(e) => Err(map_err(e)),
    }
}
