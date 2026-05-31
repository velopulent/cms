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
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }
    match services.collection.list_collections(&site_id).await {
        Ok(collections) => ok_result(&collections),
        Err(e) => Ok(tool_error(e)),
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
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .collection
        .get_collection(&site_id, &params.0.slug)
        .await
    {
        Ok(Some(collection)) => ok_result(&collection),
        Ok(None) => Ok(tool_error(crate::services::error::ServiceError::NotFound(
            "Collection not found".into(),
        ))),
        Err(e) => Ok(tool_error(e)),
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
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsWrite, "admin")
        .await
    {
        return Ok(tool_error(e));
    }
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
        Err(e) => Ok(tool_error(e)),
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
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsWrite, "admin")
        .await
    {
        return Ok(tool_error(e));
    }
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
        Err(e) => Ok(tool_error(e)),
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
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::CollectionsWrite, "admin")
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .collection
        .delete_collection(&site_id, &params.0.slug)
        .await
    {
        Ok(n) => {
            if n > 0 {
                ok_result(&serde_json::json!({"deleted": true}))
            } else {
                Ok(tool_error(crate::services::error::ServiceError::NotFound(
                    "Collection not found".into(),
                )))
            }
        }
        Err(e) => Ok(tool_error(e)),
    }
}
