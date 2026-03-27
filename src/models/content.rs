use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema)]
pub struct Content {
    pub id: String,
    pub site_id: String,
    pub schema_id: String,
    pub data: String,
    pub slug: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateContent {
    pub schema_id: String,
    pub data: serde_json::Value,
    pub slug: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateContent {
    pub data: Option<serde_json::Value>,
    pub slug: Option<String>,
    pub status: Option<String>,
}
