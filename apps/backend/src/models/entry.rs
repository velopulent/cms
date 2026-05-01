use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct Entry {
    pub id: String,
    pub site_id: String,
    pub collection_id: String,
    pub data: String,
    pub slug: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateEntry {
    pub collection_id: String,
    pub data: serde_json::Value,
    pub slug: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateEntry {
    pub data: Option<serde_json::Value>,
    pub slug: Option<String>,
    pub status: Option<String>,
    pub change_summary: Option<String>,
}

#[derive(Serialize, FromRow, Clone)]
pub struct EntryRevision {
    pub id: String,
    pub entry_id: String,
    pub revision_number: i64,
    pub data: sqlx::types::Json<serde_json::Value>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub change_summary: Option<String>,
}
