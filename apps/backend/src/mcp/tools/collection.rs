use std::sync::Arc;

use rmcp::model::CallToolResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Content;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::middleware::auth::{Principal, SCOPE_SCHEMA_READ, SCOPE_SCHEMA_WRITE};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data).unwrap_or_default();
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
    principal: &Principal,
    params: Parameters<ListCollectionsParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_READ, "viewer").await.map_err(map_err)?;
    let collections = services.collection.list_collections(&params.0.site_id).await.map_err(map_err)?;
    ok_result(&collections)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCollectionParams {
    pub site_id: String,
    pub slug: String,
}

pub async fn get_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<GetCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_READ, "viewer").await.map_err(map_err)?;
    match services.collection.get_collection(&params.0.site_id, &params.0.slug).await.map_err(map_err)? {
        Some(col) => ok_result(&col),
        None => Ok(CallToolResult::success(vec![Content::text("Collection not found")])),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCollectionParams {
    pub site_id: String,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub definition: String,
    #[serde(default)]
    pub is_singleton: bool,
}

pub async fn create_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<CreateCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_WRITE, "editor").await.map_err(map_err)?;
    let col = services.collection.create_collection(
        &params.0.site_id, &params.0.name, &params.0.slug, &params.0.definition, params.0.is_singleton,
    ).await.map_err(map_err)?;
    ok_result(&col)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateCollectionParams {
    pub site_id: String,
    pub slug: String,
    pub name: Option<String>,
    pub new_slug: Option<String>,
    pub definition: Option<String>,
}

pub async fn update_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<UpdateCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_WRITE, "editor").await.map_err(map_err)?;
    let col = services.collection.update_collection(
        &params.0.site_id, &params.0.slug,
        params.0.name.as_deref(), params.0.new_slug.as_deref(), params.0.definition.as_deref(),
    ).await.map_err(map_err)?;
    ok_result(&col)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteCollectionParams {
    pub site_id: String,
    pub slug: String,
}

pub async fn delete_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteCollectionParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_SCHEMA_WRITE, "editor").await.map_err(map_err)?;
    services.collection.delete_collection(&params.0.site_id, &params.0.slug).await.map_err(map_err)?;
    Ok(CallToolResult::success(vec![Content::text("Collection deleted")]))
}