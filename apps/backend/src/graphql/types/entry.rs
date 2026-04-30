use async_graphql::{InputObject, SimpleObject};

use super::json::Json;

#[derive(SimpleObject)]
pub struct Entry {
    pub id: String,
    pub site_id: String,
    pub collection_id: String,
    pub data: Json,
    pub slug: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(InputObject)]
pub struct CreateEntryInput {
    pub collection_id: String,
    pub data: Json,
    pub slug: String,
}

#[derive(InputObject)]
pub struct UpdateEntryInput {
    pub data: Option<Json>,
    pub slug: Option<String>,
    pub status: Option<String>,
}

pub fn db_entry_to_gql(e: crate::models::entry::Entry) -> Entry {
    let data = serde_json::from_str(&e.data).unwrap_or(serde_json::Value::Null);
    Entry {
        id: e.id,
        site_id: e.site_id,
        collection_id: e.collection_id,
        data: Json(data),
        slug: e.slug,
        status: e.status,
        created_at: e.created_at,
        updated_at: e.updated_at,
        published_at: e.published_at,
    }
}
