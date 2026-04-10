use async_graphql::SimpleObject;

use crate::graphql::context::GqlContext;

#[derive(SimpleObject)]
pub struct File {
    pub id: String,
    pub site_id: String,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub created_by: Option<String>,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct FileReference {
    pub entry_id: String,
    pub collection_name: String,
    pub field_name: String,
}

pub fn db_file_to_gql(f: crate::models::file::File, gql_ctx: &GqlContext) -> File {
    let url = match f.storage_provider.as_str() {
        "s3" => gql_ctx
            .storage
            .s3
            .as_ref()
            .map(|s| s.url(&f.storage_key))
            .unwrap_or_else(|| format!("/api/files/{}", f.id)),
        _ => format!("/api/files/{}", f.id),
    };
    let thumbnail_url = f
        .thumbnail_key
        .as_ref()
        .map(|_| format!("/api/files/{}/thumbnail", f.id));

    File {
        id: f.id,
        site_id: f.site_id,
        filename: f.filename,
        original_name: f.original_name,
        mime_type: f.mime_type,
        size: f.size,
        url,
        thumbnail_url,
        width: f.width,
        height: f.height,
        created_by: f.created_by,
        created_at: f.created_at,
    }
}
