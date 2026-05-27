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
pub struct ListCollectionsParams {
    pub site_id: String,
}

pub async fn list_collections(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListCollectionsParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::CollectionsRead, "viewer")
        .await
        .map_err(map_err)?;
    match services.collection.list_collections(&params.0.site_id).await {
        Ok(collections) => ok_result(&collections),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCollectionParams {
    pub site_id: String,
    pub collection_slug: String,
}

pub async fn get_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<GetCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::CollectionsRead, "viewer")
        .await
        .map_err(map_err)?;
    match services
        .collection
        .get_collection(&params.0.collection_slug, &params.0.site_id)
        .await
    {
        Ok(Some(collection)) => ok_result(&collection),
        Ok(None) => ok_result(&serde_json::json!({"error": "Collection not found"})),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCollectionParams {
    pub site_id: String,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
}

pub async fn create_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<CreateCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::CollectionsWrite, "admin")
        .await
        .map_err(map_err)?;
    let slug = params.0.slug.unwrap_or_else(|| params.0.name.to_lowercase().replace(' ', "-"));
    match services
        .collection
        .create_collection(&params.0.site_id, &params.0.name, &slug, params.0.description.as_deref().unwrap_or(""), false)
        .await
    {
        Ok(collection) => ok_result(&collection),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateCollectionParams {
    pub site_id: String,
    pub collection_slug: String,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub async fn update_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::CollectionsWrite, "admin")
        .await
        .map_err(map_err)?;
    match services
        .collection
        .update_collection(&params.0.site_id, &params.0.collection_slug, params.0.name.as_deref(), None, params.0.description.as_deref())
        .await
    {
        Ok(collection) => ok_result(&collection),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteCollectionParams {
    pub site_id: String,
    pub collection_slug: String,
}

pub async fn delete_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::CollectionsWrite, "admin")
        .await
        .map_err(map_err)?;
    match services
        .collection
        .delete_collection(&params.0.collection_slug, &params.0.site_id)
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
