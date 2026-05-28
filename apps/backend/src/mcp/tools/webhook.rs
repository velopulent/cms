use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;

use std::collections::HashMap;

use crate::mcp::auth::{map_err, ok_result};
use crate::middleware::auth::{Actor, Scope};
use crate::services::{Services, scope::ScopeChecker};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWebhooksParams {
    pub site_id: String,
}

pub async fn list_webhooks(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<ListWebhooksParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::WebhooksRead, "viewer")
        .await
        .map_err(map_err)?;
    let webhooks = services.webhook.list_webhooks(&params.0.site_id).await.map_err(map_err)?;
    ok_result(&webhooks)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateWebhookParams {
    pub site_id: String,
    pub label: String,
    pub url: String,
}

pub async fn create_webhook(
    scope: &Arc<ScopeChecker>,
    services: &Arc<Services>,
    actor: &Actor,
    params: Parameters<CreateWebhookParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::WebhooksWrite, "admin")
        .await
        .map_err(map_err)?;
    let user_id = actor.user_id();
    let webhook = services
        .webhook
        .create_webhook(&params.0.site_id, &params.0.label, &params.0.url, &HashMap::new(), user_id)
        .await
        .map_err(map_err)?;
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
    actor: &Actor,
    params: Parameters<TriggerWebhookParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::WebhooksWrite, "editor")
        .await
        .map_err(map_err)?;
    let user_id = actor.user_id();
    let delivery = services
        .webhook
        .trigger_webhook(&params.0.webhook_id, &params.0.site_id, user_id)
        .await
        .map_err(map_err)?;
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
    actor: &Actor,
    params: Parameters<DeleteWebhookParams>,
) -> Result<CallToolResult, McpError> {
    scope
        .require_site_scope(actor, &params.0.site_id, &Scope::WebhooksWrite, "admin")
        .await
        .map_err(map_err)?;
    services
        .webhook
        .delete_webhook(&params.0.webhook_id, &params.0.site_id)
        .await
        .map_err(map_err)?;
    ok_result(&serde_json::json!({"deleted": true}))
}
