use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::auth::{map_err, ok_result, text_result};
use crate::middleware::auth::{Principal, SCOPE_WEBHOOKS_READ, SCOPE_WEBHOOKS_TRIGGER, SCOPE_WEBHOOKS_WRITE};
use crate::services::{Services, scope::ScopeChecker};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWebhooksParams {
    #[serde(default)]
    pub site_id: Option<String>,
}

pub async fn list_webhooks(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<ListWebhooksParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_WEBHOOKS_READ, "viewer")
        .await
        .map_err(map_err)?;
    let webhooks = services.webhook.list_webhooks(&site.site_id).await.map_err(map_err)?;
    ok_result(&webhooks)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateWebhookParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub label: String,
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

pub async fn create_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<CreateWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_WEBHOOKS_WRITE, "admin")
        .await
        .map_err(map_err)?;
    let created_by = principal.user_id().unwrap_or("system");
    let webhook = services
        .webhook
        .create_webhook(
            &site.site_id,
            &params.0.label,
            &params.0.url,
            &params.0.headers,
            created_by,
        )
        .await
        .map_err(map_err)?;
    ok_result(&webhook)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerWebhookParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub webhook_id: String,
}

pub async fn trigger_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<TriggerWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_WEBHOOKS_TRIGGER, "editor")
        .await
        .map_err(map_err)?;
    let triggered_by = principal.user_id().unwrap_or("system");
    let delivery = services
        .webhook
        .trigger_webhook(&params.0.webhook_id, &site.site_id, triggered_by)
        .await
        .map_err(map_err)?;
    ok_result(&delivery)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteWebhookParams {
    #[serde(default)]
    pub site_id: Option<String>,
    pub webhook_id: String,
}

pub async fn delete_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope
        .require_site_scope(principal, params.0.site_id.as_deref(), SCOPE_WEBHOOKS_WRITE, "admin")
        .await
        .map_err(map_err)?;
    services
        .webhook
        .delete_webhook(&params.0.webhook_id, &site.site_id)
        .await
        .map_err(map_err)?;
    Ok(text_result("Webhook deleted"))
}
