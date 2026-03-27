use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema)]
pub struct ApiKey {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub key_prefix: String,
    pub permissions: String,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateApiKey {
    pub name: String,
}

#[derive(Serialize, ToSchema)]
pub struct ApiKeyResponse {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub key: String,
    pub key_prefix: String,
    pub permissions: String,
    pub created_at: String,
}
