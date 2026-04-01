use async_graphql::{InputObject, SimpleObject};

use super::json::Json;

#[derive(SimpleObject)]
pub struct Content {
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
pub struct CreateContentInput {
    pub collection_id: String,
    pub data: Json,
    pub slug: String,
}

#[derive(InputObject)]
pub struct UpdateContentInput {
    pub data: Option<Json>,
    pub slug: Option<String>,
    pub status: Option<String>,
}

pub fn db_content_to_gql(c: crate::models::content::Content) -> Content {
    let data = serde_json::from_str(&c.data).unwrap_or(serde_json::Value::Null);
    Content {
        id: c.id,
        site_id: c.site_id,
        collection_id: c.collection_id,
        data: Json(data),
        slug: c.slug,
        status: c.status,
        created_at: c.created_at,
        updated_at: c.updated_at,
        published_at: c.published_at,
    }
}
