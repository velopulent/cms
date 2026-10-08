use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{ok_result, tool_error};
use crate::mcp::schema::ArbitraryJson;
use crate::middleware::auth::Actor;
use crate::models::authorization::Action;
use crate::models::entry::PublicEntry;
use crate::repository::traits::ListEntriesParams as RepoListEntriesParams;
use crate::services::entry::UpdateEntryInput;
use crate::services::{Services, authorization::AuthorizationService};
use crate::storage::StorageRegistry;

fn public_entry(entry: crate::models::entry::Entry) -> Result<PublicEntry, McpError> {
    PublicEntry::try_from(entry).map_err(|_| McpError::internal_error("Stored entry data is invalid JSON", None))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEntriesParams {
    pub site_id: String,
    pub collection_slug: Option<String>,
    pub published_only: Option<bool>,
    pub status: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
}

pub async fn list_entries(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListEntriesParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    let published_only = params.0.published_only.unwrap_or(true);
    if let Err(e) = authorization
        .require_site_action(
            actor,
            &site_id,
            if published_only {
                Action::ContentRead
            } else {
                Action::ContentPreviewRead
            },
        )
        .await
    {
        return Ok(tool_error(e));
    }
    let page = params.0.page.unwrap_or(1).max(1);
    let per_page = params.0.per_page.unwrap_or(25).clamp(1, 100);
    let list_params = RepoListEntriesParams {
        site_id: &site_id,
        collection_slug: params.0.collection_slug.as_deref(),
        collection_id: None,
        status: params.0.status.as_deref(),
        search: params.0.search.as_deref(),
        published_only,
        page,
        per_page,
    };
    match services.entry.list_entries(list_params).await {
        Ok(result) => {
            let items: Result<Vec<_>, _> = result.items.into_iter().map(public_entry).collect();
            let response = serde_json::json!({
                "items": items?,
                "total": result.total,
                "page": result.page,
                "per_page": result.per_page,
            });
            ok_result(&response)
        }
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntryParams {
    pub site_id: String,
    pub id: String,
    pub include_drafts: Option<bool>,
}

pub async fn get_entry(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    _storage_registry: &Arc<StorageRegistry>,
    actor: &Actor,
    params: Parameters<GetEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    let include_drafts = params.0.include_drafts.unwrap_or(false);
    if let Err(e) = authorization
        .require_site_action(
            actor,
            &site_id,
            if include_drafts {
                Action::ContentPreviewRead
            } else {
                Action::ContentRead
            },
        )
        .await
    {
        return Ok(tool_error(e));
    }
    match services.entry.get_entry(&params.0.id, &site_id, !include_drafts).await {
        Ok(Some(entry)) => ok_result(&public_entry(entry)?),
        Ok(None) => Ok(tool_error(crate::services::error::ServiceError::NotFound(
            "Entry not found".into(),
        ))),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEntryParams {
    pub site_id: String,
    pub collection_id: String,
    #[schemars(with = "ArbitraryJson")]
    pub values: serde_json::Value,
    pub slug: Option<String>,
    pub published: Option<bool>,
}

pub async fn create_entry(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<CreateEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentWrite)
        .await
    {
        return Ok(tool_error(e));
    }

    if params.0.published == Some(true) {
        return Err(McpError::invalid_params(
            "Create entries as drafts, then use set_entry_publication",
            None,
        ));
    }

    let slug = params.0.slug.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    match services
        .entry
        .create_entry(
            &site_id,
            &params.0.collection_id,
            &params.0.values,
            &slug,
            actor.user_id(),
        )
        .await
    {
        Ok(entry) => ok_result(&public_entry(entry)?),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateEntryParams {
    pub site_id: String,
    pub id: String,
    #[schemars(with = "ArbitraryJson")]
    pub values: Option<serde_json::Value>,
    pub slug: Option<String>,
    pub published: Option<bool>,
    pub change_summary: Option<String>,
    pub expected_version: Option<String>,
}

pub async fn update_entry(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentWrite)
        .await
    {
        return Ok(tool_error(e));
    }
    if params.0.published.is_some()
        && let Err(error) = authorization
            .require_site_action(actor, &site_id, Action::ContentPublish)
            .await
    {
        return Ok(tool_error(error));
    }
    match services
        .entry
        .update_entry(UpdateEntryInput {
            id: &params.0.id,
            site_id: &site_id,
            data: params.0.values.as_ref(),
            slug: params.0.slug.as_deref(),
            status: params.0.published.map(|b| if b { "published" } else { "draft" }),
            created_by: actor.user_id(),
            change_summary: params.0.change_summary.as_deref(),
            expected_version: params.0.expected_version.as_deref(),
        })
        .await
    {
        Ok(entry) => ok_result(&public_entry(entry)?),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteEntryParams {
    pub site_id: String,
    pub id: String,
}

pub async fn delete_entry(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteEntryParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentWrite)
        .await
    {
        return Ok(tool_error(e));
    }
    match services.entry.delete_entry(&params.0.id, &site_id).await {
        Ok(n) => {
            if n > 0 {
                ok_result(&serde_json::json!({"deleted": true}))
            } else {
                Ok(tool_error(crate::services::error::ServiceError::NotFound(
                    "Entry not found".into(),
                )))
            }
        }
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPublicationParams {
    pub site_id: String,
    pub id: String,
    pub published: bool,
}

pub async fn set_publication(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<SetPublicationParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentPublish)
        .await
    {
        return Ok(tool_error(e));
    }
    let result = if params.0.published {
        services.entry.publish_entry(&params.0.id, &site_id).await
    } else {
        services.entry.unpublish_entry(&params.0.id, &site_id).await
    };
    match result {
        Ok(entry) => ok_result(&public_entry(entry)?),
        Err(error) => Ok(tool_error(error)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRevisionsParams {
    pub site_id: String,
    pub entry_id: String,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_revisions(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListRevisionsParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentPreviewRead)
        .await
    {
        return Ok(tool_error(e));
    }
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
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRevisionParams {
    pub site_id: String,
    pub entry_id: String,
    pub revision_number: i64,
}

pub async fn get_revision(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<GetRevisionParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentPreviewRead)
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .entry
        .get_revision(&params.0.entry_id, &site_id, params.0.revision_number)
        .await
    {
        Ok(Some(revision)) => ok_result(&revision),
        Ok(None) => Ok(tool_error(crate::services::error::ServiceError::NotFound(
            "Revision not found".into(),
        ))),
        Err(error) => Ok(tool_error(error)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestoreRevisionParams {
    pub site_id: String,
    pub entry_id: String,
    pub revision_number: i64,
}

pub async fn restore_revision(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<RestoreRevisionParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentWrite)
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .entry
        .restore_revision(&params.0.entry_id, &site_id, params.0.revision_number, actor.user_id())
        .await
    {
        Ok(entry) => ok_result(&public_entry(entry)?),
        Err(e) => Ok(tool_error(e)),
    }
}
