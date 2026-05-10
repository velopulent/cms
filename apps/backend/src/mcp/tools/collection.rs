use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{map_err, ok_result, text_result};
use crate::middleware::auth::{Principal, SCOPE_SCHEMA_READ, SCOPE_SCHEMA_WRITE};
use crate::services::{Services, scope::ScopeChecker};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCollectionsParams {
    #[serde(default)]
    pub site_id: Option<String>,
}

pub async fn list_collections(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<ListCollectionsParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_READ, "viewer")
        .await
        .map_err(map_err)?;
    let collections = services
        .collection
        .list_collections(&site.site_id)
        .await
        .map_err(map_err)?;
    ok_result(&collections)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCollectionParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub slug: String,
}

pub async fn get_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<GetCollectionParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_READ, "viewer")
        .await
        .map_err(map_err)?;
    match services
        .collection
        .get_collection(&site.site_id, &params.0.slug)
        .await
        .map_err(map_err)?
    {
        Some(col) => ok_result(&col),
        None => Ok(text_result("Collection not found")),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCollectionParams {
    #[serde(default)]
    pub site_id: Option<String>,
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
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_WRITE, "editor")
        .await
        .map_err(map_err)?;
    let col = services
        .collection
        .create_collection(
            &site.site_id,
            &params.0.name,
            &params.0.slug,
            &params.0.definition,
            params.0.is_singleton,
        )
        .await
        .map_err(map_err)?;
    ok_result(&col)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateCollectionParams {
    #[serde(default)]
    pub site_id: Option<String>,
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
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_WRITE, "editor")
        .await
        .map_err(map_err)?;
    let col = services
        .collection
        .update_collection(
            &site.site_id,
            &params.0.slug,
            params.0.name.as_deref(),
            params.0.new_slug.as_deref(),
            params.0.definition.as_deref(),
        )
        .await
        .map_err(map_err)?;
    ok_result(&col)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteCollectionParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub slug: String,
}

pub async fn delete_collection(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteCollectionParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_SCHEMA_WRITE, "editor")
        .await
        .map_err(map_err)?;
    services
        .collection
        .delete_collection(&site.site_id, &params.0.slug)
        .await
        .map_err(map_err)?;
    Ok(text_result("Collection deleted"))
}
