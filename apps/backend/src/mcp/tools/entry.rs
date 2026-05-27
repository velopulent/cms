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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEntriesParams {
    pub site_id: String,
    pub collection_slug: Option<String>,
    pub published_only: Option<bool>,
    pub locale: Option<String>,
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
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::EntriesRead, "viewer")
        .await
        .map_err(map_err)?;
    let published_only = matches!(actor, Actor::ApiKey(_)) || params.0.published_only.unwrap_or(false);
    let page = params.0.page.unwrap_or(1).max(1);
    let per_page = params.0.per_page.unwrap_or(25).clamp(1, 100);
    let list_params = RepoListEntriesParams {
        site_id: &params.0.site_id,
        collection_slug: params.0.collection_slug.as_deref(),
        collection_id: None,
        status: None,
        search: params.0.search.as_deref(),
        published_only,
        page,
        per_page,
    };
    match services
        .entry
        .list_entries(list_params)
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
pub struct GetEntryParams {
    pub site_id: String,
    pub id: String,
}

pub async fn get_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    _storage_registry: &Arc<StorageRegistry>,
    actor: &Actor,
    params: Parameters<GetEntryParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::EntriesRead, "viewer")
        .await
        .map_err(map_err)?;
    let published_only = matches!(actor, Actor::ApiKey(_));
    match services
        .entry
        .get_entry(&params.0.id, &params.0.site_id, published_only)
        .await
    {
        Ok(Some(entry)) => ok_result(&entry),
        Ok(None) => ok_result(&serde_json::json!({"error": "Entry not found"})),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEntryParams {
    pub site_id: String,
    pub collection_slug: String,
    #[schemars(with = "ArbitraryJson")]
    pub values: serde_json::Value,
    pub published: Option<bool>,
    pub locale: Option<String>,
}

pub async fn create_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<CreateEntryParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    let user_id = actor.user_id().unwrap_or("system");
    let slug = uuid::Uuid::now_v7().to_string();
    match services
        .entry
        .create_entry(
            &params.0.site_id,
            &params.0.collection_slug,
            &params.0.values,
            &slug,
            Some(user_id),
        )
        .await
    {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateEntryParams {
    pub site_id: String,
    pub id: String,
    #[schemars(with = "ArbitraryJson")]
    pub values: Option<serde_json::Value>,
    pub published: Option<bool>,
    pub locale: Option<String>,
}

pub async fn update_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateEntryParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    let user_id = actor.user_id().unwrap_or("system");
    match services
        .entry
        .update_entry(
            &params.0.id,
            &params.0.site_id,
            params.0.values.as_ref(),
            None,
            params.0.published.map(|b| if b { "published" } else { "draft" }),
            Some(user_id),
            params.0.locale.as_deref(),
        )
        .await
    {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteEntryParams {
    pub site_id: String,
    pub id: String,
}

pub async fn delete_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteEntryParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services.entry.delete_entry(&params.0.id, &params.0.site_id).await {
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
    pub site_id: String,
    pub id: String,
}

pub async fn publish_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<PublishEntryParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services.entry.publish_entry(&params.0.id, &params.0.site_id).await {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnpublishEntryParams {
    pub site_id: String,
    pub id: String,
}

pub async fn unpublish_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UnpublishEntryParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::EntriesWrite, "editor")
        .await
        .map_err(map_err)?;
    match services.entry.unpublish_entry(&params.0.id, &params.0.site_id).await {
        Ok(entry) => ok_result(&entry),
        Err(e) => Err(map_err(e)),
    }
}
