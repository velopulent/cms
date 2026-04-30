use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct File {
    pub id: String,
    pub site_id: String,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub storage_provider: String,
    pub storage_key: String,
    pub thumbnail_key: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub deleted_at: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, ToSchema)]
pub struct FileWithUrl {
    pub id: String,
    pub site_id: String,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub storage_provider: String,
    pub storage_key: String,
    pub thumbnail_key: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub deleted_at: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub url: String,
    pub thumbnail_url: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct FileReference {
    pub entry_id: String,
    pub collection_name: String,
    pub field_name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct BatchFileIds {
    pub ids: Vec<String>,
}
