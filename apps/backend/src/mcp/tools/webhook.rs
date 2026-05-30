use std::collections::HashMap;
use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{map_err, ok_result};
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
    scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksRead, "viewer")
        .await
        .map_err(map_err)?;
    let webhooks = services.webhook.list_webhooks(&site_id).await.map_err(map_err)?;
    ok_result(&webhooks)
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
    scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksRead, "viewer")
        .await
        .map_err(map_err)?;
    match services
        .webhook
        .get_webhook(&params.0.webhook_id, &site_id)
        .await
    {
        Ok(Some(webhook)) => ok_result(&webhook),
        Ok(None) => ok_result(&serde_json::json!({"error": "Webhook not found"})),
        Err(e) => Err(map_err(e)),
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
    scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "admin")
        .await
        .map_err(map_err)?;
    let headers: HashMap<String, String> = params
        .0
        .headers
        .and_then(|h| serde_json::from_value(h).ok())
        .unwrap_or_default();
    let webhook = services
        .webhook
        .create_webhook(&site_id, &params.0.label, &params.0.url, &headers, None)
        .await
        .map_err(map_err)?;
    ok_result(&webhook)
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
    scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "admin")
        .await
        .map_err(map_err)?;
    let headers: Option<HashMap<String, String>> = params
        .0
        .headers
        .and_then(|h| serde_json::from_value(h).ok());
    let webhook = services
        .webhook
        .update_webhook(
            &params.0.webhook_id,
            &site_id,
            params.0.label.as_deref(),
            params.0.url.as_deref(),
            headers.as_ref(),
        )
        .await
        .map_err(map_err)?;
    ok_result(&webhook)
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
    scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "editor")
        .await
        .map_err(map_err)?;
    let delivery = services
        .webhook
        .trigger_webhook(&params.0.webhook_id, &site_id, None)
        .await
        .map_err(map_err)?;
    ok_result(&delivery)
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
    scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksWrite, "admin")
        .await
        .map_err(map_err)?;
    services
        .webhook
        .delete_webhook(&params.0.webhook_id, &site_id)
        .await
        .map_err(map_err)?;
    ok_result(&serde_json::json!({"deleted": true}))
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
    scope
        .require_site_scope(actor, &site_id, &Scope::WebhooksRead, "viewer")
        .await
        .map_err(map_err)?;
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
        Err(e) => Err(map_err(e)),
    }
}
