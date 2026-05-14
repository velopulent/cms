use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{map_err, ok_result, text_result};
use crate::mcp::schema::ArbitraryJson;
use crate::middleware::auth::{Principal, SCOPE_CONTENT_READ, SCOPE_CONTENT_WRITE};
use crate::repository::traits::ListEntriesParams as RepoListEntriesParams;
use crate::services::{Services, scope::ScopeChecker};
use crate::storage::StorageRegistry;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEntriesParams {
    #[serde(default)]
    pub site_id: Option<String>,
    #[serde(default)]
    pub collection_slug: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}

pub async fn list_entries(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<ListEntriesParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_CONTENT_READ, "viewer")
        .await
        .map_err(map_err)?;

    let published_only = matches!(principal, Principal::SiteToken { .. });
    let page = params.0.page.max(1);
    let per_page = params.0.per_page.clamp(1, 200);

    let list_params = RepoListEntriesParams {
        site_id: &site.site_id,
        collection_slug: params.0.collection_slug.as_deref(),
        collection_id: None,
        status: if published_only {
            None
        } else {
            params.0.status.as_deref()
        },
        search: params.0.search.as_deref(),
        published_only,
        page,
        per_page,
    };

    let result = services.entry.list_entries(list_params).await.map_err(map_err)?;
    let items = services.entry.resolve_entries_list_files(&result.items).await;
    ok_result(&serde_json::json!({
        "items": items,
        "total": result.total,
        "page": result.page,
        "per_page": result.per_page,
    }))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntryParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub entry_id: String,
}

pub async fn get_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    storage_registry: &Arc<StorageRegistry>,
    principal: &Principal,
    params: Parameters<GetEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_CONTENT_READ, "viewer")
        .await
        .map_err(map_err)?;

    let published_only = matches!(principal, Principal::SiteToken { .. });

    match services
        .entry
        .get_entry(&params.0.entry_id, &site.site_id, published_only)
        .await
        .map_err(map_err)?
    {
        Some(entry) => {
            let storage_provider = services
                .file
                .get_storage_provider(&site.site_id)
                .await
                .map_err(map_err)?;
            let storage = storage_registry
                .get(&storage_provider)
                .ok_or_else(|| McpError::internal_error("Storage not configured", None))?;
            let resolved = services
                .entry
                .resolve_entry_files(&entry, storage)
                .await
                .unwrap_or_else(|_| {
                    serde_json::from_str(&entry.data).unwrap_or_else(|e| {
                        tracing::warn!("Failed to parse entry.data as JSON: {}", e);
                        serde_json::Value::Object(Default::default())
                    })
                });
            ok_result(&resolved)
        }
        None => Ok(text_result("Entry not found")),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEntryParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub collection_id: String,
    #[schemars(with = "ArbitraryJson")]
    pub data: serde_json::Value,
    #[serde(default)]
    pub slug: Option<String>,
}

pub async fn create_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<CreateEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_CONTENT_WRITE, "editor")
        .await
        .map_err(map_err)?;

    let created_by = principal.user_id();
    let entry = services
        .entry
        .create_entry(
            &site.site_id,
            &params.0.collection_id,
            &params.0.data,
            params.0.slug.as_deref().unwrap_or(""),
            created_by,
        )
        .await
        .map_err(map_err)?;

    ok_result(&entry)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateEntryParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub entry_id: String,
    #[serde(default)]
    #[schemars(with = "ArbitraryJson")]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

pub async fn update_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<UpdateEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_CONTENT_WRITE, "editor")
        .await
        .map_err(map_err)?;

    let created_by = principal.user_id();
    let entry = services
        .entry
        .update_entry(
            &params.0.entry_id,
            &site.site_id,
            params.0.data.as_ref(),
            params.0.slug.as_deref(),
            params.0.status.as_deref(),
            created_by,
            None,
        )
        .await
        .map_err(map_err)?;

    ok_result(&entry)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteEntryParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub entry_id: String,
}

pub async fn delete_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_CONTENT_WRITE, "editor")
        .await
        .map_err(map_err)?;

    services
        .entry
        .delete_entry(&params.0.entry_id, &site.site_id)
        .await
        .map_err(map_err)?;
    Ok(text_result("Entry deleted"))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PublishEntryParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub entry_id: String,
}

pub async fn publish_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<PublishEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_CONTENT_WRITE, "editor")
        .await
        .map_err(map_err)?;

    let entry = services
        .entry
        .publish_entry(&params.0.entry_id, &site.site_id)
        .await
        .map_err(map_err)?;
    ok_result(&entry)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnpublishEntryParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub entry_id: String,
}

pub async fn unpublish_entry(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<UnpublishEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_CONTENT_WRITE, "editor")
        .await
        .map_err(map_err)?;

    let entry = services
        .entry
        .unpublish_entry(&params.0.entry_id, &site.site_id)
        .await
        .map_err(map_err)?;
    ok_result(&entry)
}
