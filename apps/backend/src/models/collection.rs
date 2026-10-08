use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, FromRow, ToSchema, Clone)]
pub struct Collection {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: String,
    pub is_singleton: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, ToSchema, Clone)]
pub struct PublicCollection {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: serde_json::Value,
    pub is_singleton: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<Collection> for PublicCollection {
    type Error = String;

    fn try_from(collection: Collection) -> Result<Self, Self::Error> {
        let definition = serde_json::from_str(&collection.definition)
            .map_err(|error| format!("stored collection definition is invalid JSON: {error}"))?;
        Ok(Self {
            id: collection.id,
            site_id: collection.site_id,
            name: collection.name,
            slug: collection.slug,
            definition,
            is_singleton: collection.is_singleton,
            created_at: collection.created_at,
            updated_at: collection.updated_at,
        })
    }
}

#[derive(Deserialize, ToSchema)]
pub struct CreateCollection {
    pub name: String,
    pub slug: String,
    pub definition: serde_json::Value,
    pub is_singleton: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateCollection {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub definition: Option<serde_json::Value>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateSingletonData {
    pub data: serde_json::Value,
    pub change_summary: Option<String>,
    pub expected_version: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SingletonResponse {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: serde_json::Value,
    pub data: Option<serde_json::Value>,
    pub entry_id: Option<String>,
    pub version: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
