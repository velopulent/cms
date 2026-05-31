use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::schema::ArbitraryJson;
use crate::middleware::auth::{Actor, Scope};
use crate::repository::traits::ListEntriesParams as RepoListEntriesParams;
use crate::services::{Services, scope::ScopeChecker};
use crate::storage::StorageRegistry;

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize response: {}", e), None)
    })?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_err(e: impl Into<crate::services::error::ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

fn require_site_id(actor: &Actor) -> Result<String, McpError> {
    actor
        .bound_site_id()
        .map(String::from)
        .ok_or_else(|| McpError::invalid_request("No site context", None))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEntriesParams {
    pub collection_slug: Option<String>,
    pub published_only: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
}

pub async fn list_entries(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListEntriesParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesRead, "viewer")
        .await
        .map_err(map_err)?;
    let published_only = params.0.published_only.unwrap_or(true);
    let page = params.0.page.unwrap_or(1).max(1);
    let per_page = params.0.per_page.unwrap_or(25).clamp(1, 100);
    let list_params = RepoListEntriesParams {
        site_id: &site_id,
        collection_slug: params.0.collection_slug.as_deref(),
        collection_id: None,
        status: None,
        search: params.0.search.as_deref(),
        published_only,
        page,
        per_page,
    };
    match services.entry.list_entries(list_params).await {
        Ok(result) => {
            let response = serde_json::json!({
                "items": result.items,
                "total": result.total,
                "page": result.page,
                "per_page": result.per_page,
            });
            ok_result(&response)
        }
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntryParams {
    pub id: String,
}

pub async fn get_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    _storage_registry: &Arc<StorageRegistry>,
    actor: &Actor,
    params: Parameters<GetEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesRead, "viewer")
        .await
        .map_err(map_err)?;
    match services.entry.get_entry(&params.0.id, &site_id, true).await {
        Ok(Some(entry)) => ok_result(&entry),
        Ok(None) => ok_result(&serde_json::json!({"error": "Entry not found"})),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEntryParams {
    pub collection_id: String,
    #[schemars(with = "ArbitraryJson")]
    pub values: serde_json::Value,
    pub slug: Option<String>,
    pub published: Option<bool>,
}

pub async fn create_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<CreateEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;

    // Validate entry data against collection definition
    if let Ok(Some(collection)) = services
        .collection
        .get_by_id(&params.0.collection_id)
        .await
    {
        if let Ok(definition) = serde_json::from_str::<serde_json::Value>(&collection.definition) {
            if let Some(fields) = definition.get("fields").and_then(|f| f.as_array()) {
                if let Some(err) = crate::services::definition_validation::validate_entry_data(&params.0.values, fields) {
                    return Err(McpError::invalid_request(err, None));
                }
            }
        }
    }

    let slug = params
        .0
        .slug
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    match services
        .entry
        .create_entry(&site_id, &params.0.collection_id, &params.0.values, &slug, None)
        .await
    {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateEntryParams {
    pub id: String,
    #[schemars(with = "ArbitraryJson")]
    pub values: Option<serde_json::Value>,
    pub slug: Option<String>,
    pub published: Option<bool>,
    pub change_summary: Option<String>,
}

pub async fn update_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services
        .entry
        .update_entry(
            &params.0.id,
            &site_id,
            params.0.values.as_ref(),
            params.0.slug.as_deref(),
            params.0.published.map(|b| if b { "published" } else { "draft" }),
            None,
            params.0.change_summary.as_deref(),
        )
        .await
    {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteEntryParams {
    pub id: String,
}

pub async fn delete_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services.entry.delete_entry(&params.0.id, &site_id).await {
        Ok(n) => {
            if n > 0 {
                ok_result(&serde_json::json!({"deleted": true}))
            } else {
                ok_result(&serde_json::json!({"error": "Entry not found"}))
            }
        }
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PublishEntryParams {
    pub id: String,
}

pub async fn publish_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<PublishEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services.entry.publish_entry(&params.0.id, &site_id).await {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnpublishEntryParams {
    pub id: String,
}

pub async fn unpublish_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UnpublishEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services.entry.unpublish_entry(&params.0.id, &site_id).await {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRevisionsParams {
    pub entry_id: String,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_revisions(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListRevisionsParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesRead, "viewer")
        .await
        .map_err(map_err)?;
    let page = params.0.page.unwrap_or(1).max(1);
    let per_page = params.0.per_page.unwrap_or(50).clamp(1, 200);
    match services
        .entry
        .list_revisions(&params.0.entry_id, &site_id, page, per_page)
        .await
    {
        Ok(result) => {
            let response = serde_json::json!({
                "items": result.items,
                "total": result.total,
                "page": result.page,
                "per_page": result.per_page,
            });
            ok_result(&response)
        }
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestoreRevisionParams {
    pub entry_id: String,
    pub revision_number: i64,
}

pub async fn restore_revision(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<RestoreRevisionParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    scope
        .require_site_scope(actor, &site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services
        .entry
        .restore_revision(&params.0.entry_id, &site_id, params.0.revision_number, None)
        .await
    {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}
