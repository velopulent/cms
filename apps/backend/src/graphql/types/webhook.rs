use async_graphql::SimpleObject;
use std::collections::HashMap;

#[derive(SimpleObject)]
pub struct SiteWebhook {
    pub id: String,
    pub site_id: String,
    pub label: String,
    pub url: String,
    pub headers: Option<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct WebhookDelivery {
    pub id: String,
    pub webhook_id: String,
    pub status: String,
    pub status_code: Option<i32>,
    pub response_body: Option<String>,
    pub duration_ms: Option<i64>,
    pub triggered_by: String,
    pub triggered_at: String,
}

pub fn db_webhook_to_gql(
    webhook: crate::models::webhook::SiteWebhook,
    headers: HashMap<String, String>,
) -> SiteWebhook {
    SiteWebhook {
        id: webhook.id,
        site_id: webhook.site_id,
        label: webhook.label,
        url: webhook.url,
        headers: Some(serde_json::to_string(&headers).unwrap_or_default()),
        created_by: webhook.created_by,
        created_at: webhook.created_at,
        updated_at: webhook.updated_at,
    }
}

pub fn db_delivery_to_gql(delivery: crate::models::webhook::WebhookDelivery) -> WebhookDelivery {
    WebhookDelivery {
        id: delivery.id,
        webhook_id: delivery.webhook_id,
        status: delivery.status,
        status_code: delivery.status_code,
        response_body: delivery.response_body,
        duration_ms: delivery.duration_ms,
        triggered_by: delivery.triggered_by,
        triggered_at: delivery.triggered_at,
    }
}