use std::sync::Arc;

use rmcp::model::CallToolResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::Content;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::middleware::auth::{Principal, SCOPE_WEBHOOKS_READ, SCOPE_WEBHOOKS_WRITE, SCOPE_WEBHOOKS_TRIGGER};
use crate::services::{Services, error::ServiceError, scope::ScopeChecker};

fn ok_result(data: &impl serde::Serialize) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string_pretty(data).unwrap_or_default();
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn map_err(e: impl Into<ServiceError>) -> McpError {
    crate::mcp::auth::service_error_to_mcp(e.into())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWebhooksParams {
    pub site_id: String,
}

pub async fn list_webhooks(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<ListWebhooksParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_admin_scope(principal, Some(&params.0.site_id), SCOPE_WEBHOOKS_READ).await.map_err(map_err)?;
    let webhooks = services.webhook.list_webhooks(&params.0.site_id).await.map_err(map_err)?;
    ok_result(&webhooks)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateWebhookParams {
    pub site_id: String,
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
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_WEBHOOKS_WRITE, "admin")
        .await.map_err(map_err)?;
    let created_by = principal.user_id().unwrap_or("system");
    let webhook = services.webhook.create_webhook(
        &site.site_id, &params.0.label, &params.0.url, &params.0.headers, created_by,
    ).await.map_err(map_err)?;
    ok_result(&webhook)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerWebhookParams {
    pub site_id: String,
    pub webhook_id: String,
}

pub async fn trigger_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<TriggerWebhookParams>,
) -> Result<CallToolResult, McpError> {
    scope.require_admin_scope(principal, Some(&params.0.site_id), SCOPE_WEBHOOKS_TRIGGER).await.map_err(map_err)?;
    let triggered_by = principal.user_id().unwrap_or("system");
    let delivery = services.webhook.trigger_webhook(
        &params.0.webhook_id, &params.0.site_id, triggered_by,
    ).await.map_err(map_err)?;
    ok_result(&delivery)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteWebhookParams {
    pub site_id: String,
    pub webhook_id: String,
}

pub async fn delete_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    principal: &Principal,
    params: Parameters<DeleteWebhookParams>,
) -> Result<CallToolResult, McpError> {
    let site = scope.require_site_scope(principal, Some(&params.0.site_id), SCOPE_WEBHOOKS_WRITE, "admin")
        .await.map_err(map_err)?;
    services.webhook.delete_webhook(&params.0.webhook_id, &site.site_id).await.map_err(map_err)?;
    Ok(CallToolResult::success(vec![Content::text("Webhook deleted")]))
}