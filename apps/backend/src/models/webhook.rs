use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct SiteWebhook {
    pub id: String,
    pub site_id: String,
    pub label: String,
    pub url: String,
    pub headers_encrypted: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateWebhook {
    pub label: String,
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateWebhook {
    pub label: Option<String>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize, FromRow, ToSchema, Clone)]
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