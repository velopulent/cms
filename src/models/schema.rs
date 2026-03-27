use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema)]
pub struct Schema {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateSchema {
    pub name: String,
    pub slug: String,
    pub definition: serde_json::Value,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateSchema {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub definition: Option<serde_json::Value>,
}
