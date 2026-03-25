use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, FromRow)]
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

#[derive(Deserialize)]
pub struct CreateContent {
    pub schema_id: String,
    pub data: serde_json::Value,
    pub slug: String,
}

#[derive(Deserialize)]
pub struct UpdateContent {
    pub data: Option<serde_json::Value>,
    pub slug: Option<String>,
    pub status: Option<String>,
}
