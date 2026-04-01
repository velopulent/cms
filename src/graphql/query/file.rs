use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::file::*;

pub struct FileQuery;

#[Object]
impl FileQuery {
    async fn files(
        &self,
        ctx: &Context<'_>,
        page: Option<i64>,
        search: Option<String>,
        file_type: Option<String>,
    ) -> Result<Vec<File>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let page = page.unwrap_or(1).max(1);
        let per_page: i64 = 30;
        let offset = (page - 1) * per_page;

        let mut query = String::from(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at
             FROM files WHERE site_id = ? AND deleted_at IS NULL",
        );
        let mut bindings: Vec<String> = vec![site_id.to_string()];

        if let Some(s) = &search {
            query.push_str(" AND (original_name LIKE ? OR filename LIKE ?)");
            let pattern = format!("%{}%", s);
            bindings.push(pattern.clone());
            bindings.push(pattern);
        }

        if let Some(ft) = &file_type {
            match ft.as_str() {
                "image" => query.push_str(" AND mime_type LIKE 'image/%'"),
                "video" => query.push_str(" AND mime_type LIKE 'video/%'"),
                "document" => query.push_str(
                    " AND (mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/%' OR mime_type LIKE 'text/%')",
                ),
                _ => {}
            }
        }

        query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        bindings.push(per_page.to_string());
        bindings.push(offset.to_string());

        let mut q = sqlx::query_as::<_, crate::models::file::File>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let db_files = q
            .fetch_all(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_files.into_iter().map(|f| db_file_to_gql(f, gql_ctx)).collect())
    }

    async fn file(&self, ctx: &Context<'_>, id: String) -> Result<File> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_file = sqlx::query_as::<_, crate::models::file::File>(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at
             FROM files WHERE id = ? AND site_id = ? AND deleted_at IS NULL",
        )
        .bind(&id)
        .bind(site_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match db_file {
            Some(f) => Ok(db_file_to_gql(f, gql_ctx)),
            None => Err(async_graphql::Error::new("File not found")),
        }
    }

    async fn file_references(&self, ctx: &Context<'_>, file_id: String) -> Result<Vec<FileReference>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT DISTINCT c.id, col.name FROM content_file_references cfr
             JOIN content c ON cfr.content_id = c.id
             JOIN collections col ON c.collection_id = col.id
             WHERE cfr.file_id = ? AND c.site_id = ?",
        )
        .bind(&file_id)
        .bind(site_id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(rows
            .into_iter()
            .map(|(content_id, collection_name)| FileReference {
                content_id,
                collection_name,
                field_name: String::new(),
            })
            .collect())
    }
}
