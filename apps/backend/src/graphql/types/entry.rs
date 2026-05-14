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
    pub change_summary: Option<String>,
}

#[derive(SimpleObject)]
pub struct EntryRevision {
    pub id: String,
    pub entry_id: String,
    pub revision_number: i64,
    pub data: Json,
    pub created_by: Option<String>,
    pub created_at: String,
    pub change_summary: Option<String>,
    pub diff_from_previous: Option<Json>,
}

#[derive(SimpleObject)]
pub struct RevisionsListResult {
    pub items: Vec<EntryRevision>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
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

pub fn db_revision_to_gql(r: crate::models::entry::EntryRevision, diff: Option<serde_json::Value>) -> EntryRevision {
    EntryRevision {
        id: r.id,
        entry_id: r.entry_id,
        revision_number: r.revision_number,
        data: Json(r.data.0),
        created_by: r.created_by,
        created_at: r.created_at,
        change_summary: r.change_summary,
        diff_from_previous: diff.map(Json),
    }
}
