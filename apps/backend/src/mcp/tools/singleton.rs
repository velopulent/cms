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
use crate::services::{Services, authorization::AuthorizationService};
use crate::storage::StorageRegistry;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSingletonsParams {
    pub site_id: String,
}

pub async fn list_singletons(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListSingletonsParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id;
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentRead)
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
    pub site_id: String,
    pub slug: String,
}

pub async fn get_singleton(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    storage_registry: &Arc<StorageRegistry>,
    actor: &Actor,
    params: Parameters<GetSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentRead)
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
        None => {
            return Ok(tool_error(crate::services::error::ServiceError::Internal(
                "Storage not configured".into(),
            )));
        }
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
    pub site_id: String,
    pub slug: String,
    #[schemars(with = "ArbitraryJson")]
    pub data: serde_json::Value,
    pub change_summary: Option<String>,
}

pub async fn update_singleton(
    authorization: &Arc<AuthorizationService>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateSingletonParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = params.0.site_id.clone();
    if let Err(e) = authorization
        .require_site_action(actor, &site_id, Action::ContentWrite)
        .await
    {
        return Ok(tool_error(e));
    }

    let created_by = actor.user_id();
    match services
        .singleton
        .update_singleton(
            &site_id,
            &params.0.slug,
            &params.0.data,
            created_by,
            params.0.change_summary.as_deref(),
        )
        .await
    {
        Ok(singleton) => ok_result(&singleton),
        Err(e) => Ok(tool_error(e)),
    }
}
