use std::collections::HashMap;
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
pub struct ListWebhooksParams {}

pub async fn list_webhooks(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    _params: Parameters<ListWebhooksParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }
    match services.webhook.list_webhooks(&site_id).await {
        Ok(webhooks) => ok_result(&webhooks),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetWebhookParams {
    pub webhook_id: String,
}

pub async fn get_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<GetWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .webhook
        .get_webhook(&params.0.webhook_id, &site_id)
        .await
    {
        Ok(Some(webhook)) => ok_result(&webhook),
        Ok(None) => Ok(tool_error(crate::services::error::ServiceError::NotFound(
            "Webhook not found".into(),
        ))),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateWebhookParams {
    pub label: String,
    pub url: String,
    #[schemars(with = "ArbitraryJson")]
    pub headers: Option<serde_json::Value>,
}

pub async fn create_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<CreateWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "admin")
        .await
    {
        return Ok(tool_error(e));
    }
    let headers: HashMap<String, String> = params
        .0
        .headers
        .and_then(|h| serde_json::from_value(h).ok())
        .unwrap_or_default();
    match services
        .webhook
        .create_webhook(&site_id, &params.0.label, &params.0.url, &headers, None)
        .await
    {
        Ok(webhook) => ok_result(&webhook),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateWebhookParams {
    pub webhook_id: String,
    pub label: Option<String>,
    pub url: Option<String>,
    #[schemars(with = "ArbitraryJson")]
    pub headers: Option<serde_json::Value>,
}

pub async fn update_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<UpdateWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "admin")
        .await
    {
        return Ok(tool_error(e));
    }
    let headers: Option<HashMap<String, String>> = params
        .0
        .headers
        .and_then(|h| serde_json::from_value(h).ok());
    match services
        .webhook
        .update_webhook(
            &params.0.webhook_id,
            &site_id,
            params.0.label.as_deref(),
            params.0.url.as_deref(),
            headers.as_ref(),
        )
        .await
    {
        Ok(webhook) => ok_result(&webhook),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerWebhookParams {
    pub webhook_id: String,
}

pub async fn trigger_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<TriggerWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "editor")
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .webhook
        .trigger_webhook(&params.0.webhook_id, &site_id, None)
        .await
    {
        Ok(delivery) => ok_result(&delivery),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteWebhookParams {
    pub webhook_id: String,
}

pub async fn delete_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<DeleteWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "admin")
        .await
    {
        return Ok(tool_error(e));
    }
    match services
        .webhook
        .delete_webhook(&params.0.webhook_id, &site_id)
        .await
    {
        Ok(_) => ok_result(&serde_json::json!({"deleted": true})),
        Err(e) => Ok(tool_error(e)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWebhookDeliveriesParams {
    pub webhook_id: String,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub async fn list_webhook_deliveries(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListWebhookDeliveriesParams>,
) -> Result<CallToolResult, McpError> {
    let site_id = require_site_id(actor)?;
    if let Err(e) = scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksRead, "viewer")
        .await
    {
        return Ok(tool_error(e));
    }
    let page = params.0.page.unwrap_or(1).max(1);
    let per_page = params.0.per_page.unwrap_or(50).clamp(1, 200);
    match services
        .webhook
        .list_deliveries(&params.0.webhook_id, &site_id, page, per_page)
        .await
    {
        Ok((deliveries, total)) => {
            let response = serde_json::json!({
                "items": deliveries,
                "total": total,
                "page": page,
                "per_page": per_page,
            });
            ok_result(&response)
        }
        Err(e) => Ok(tool_error(e)),
    }
}
