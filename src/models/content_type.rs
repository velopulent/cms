use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, FromRow)]
pub struct ContentType {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub schema_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct CreateContentType {
    pub name: String,
    pub slug: String,
    pub schema_json: serde_json::Value,
}

#[derive(Deserialize)]
pub struct UpdateContentType {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub schema_json: Option<serde_json::Value>,
}
