use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::collection::Collection;
use super::types::content::Content;
use super::types::file::File;
use super::types::site::Site;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn site(&self, ctx: &Context<'_>) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_site = sqlx::query_as::<_, crate::models::site::Site>(
            "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
        )
        .bind(site_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match db_site {
            Some(s) => Ok(Site {
                id: s.id,
                name: s.name,
                default_storage_provider: s.default_storage_provider,
                created_by: s.created_by,
                created_at: s.created_at,
                updated_at: s.updated_at,
            }),
            None => Err(async_graphql::Error::new("Site not found")),
        }
    }

    async fn collections(&self, ctx: &Context<'_>) -> Result<Vec<Collection>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_collections = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? ORDER BY name",
        )
        .bind(site_id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_collections
            .into_iter()
            .map(super::types::collection::db_collection_to_gql)
            .collect())
    }

    async fn collection(&self, ctx: &Context<'_>, slug: String) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_collection = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
        )
        .bind(site_id)
        .bind(&slug)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match db_collection {
            Some(c) => Ok(super::types::collection::db_collection_to_gql(c)),
            None => Err(async_graphql::Error::new("Collection not found")),
        }
    }

    async fn content(
        &self,
        ctx: &Context<'_>,
        collection_id: Option<String>,
        status: Option<String>,
        r#type: Option<String>,
        search: Option<String>,
    ) -> Result<Vec<Content>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let mut query = String::from(
            "SELECT c.id, c.site_id, c.collection_id, c.data, c.slug, c.status, c.created_at, c.updated_at, c.published_at
             FROM content c
             JOIN collections col ON c.collection_id = col.id
             WHERE c.site_id = ?",
        );
        let mut bindings: Vec<String> = vec![site_id.to_string()];

        if let Some(cid) = collection_id {
            query.push_str(" AND c.collection_id = ?");
            bindings.push(cid);
        }

        if let Some(content_type) = &r#type {
            query.push_str(" AND col.slug = ?");
            bindings.push(content_type.clone());
        }

        if let Some(s) = &status {
            query.push_str(" AND c.status = ?");
            bindings.push(s.clone());
        } else {
            query.push_str(" AND c.status = 'published'");
        }

        if let Some(s) = &search {
            query.push_str(" AND c.data LIKE ?");
            bindings.push(format!("%{}%", s));
        }

        query.push_str(" ORDER BY c.updated_at DESC");

        let mut q = sqlx::query_as::<_, crate::models::content::Content>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let items = q
            .fetch_all(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(items
            .into_iter()
            .map(super::types::content::db_content_to_gql)
            .collect())
    }

    async fn content_item(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let content = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
             FROM content WHERE id = ? AND site_id = ?",
        )
        .bind(&id)
        .bind(site_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match content {
            Some(c) => Ok(super::types::content::db_content_to_gql(c)),
            None => Err(async_graphql::Error::new("Content not found")),
        }
    }

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

        Ok(db_files
            .into_iter()
            .map(|f| super::types::file::db_file_to_gql(f, gql_ctx))
            .collect())
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
            Some(f) => Ok(super::types::file::db_file_to_gql(f, gql_ctx)),
            None => Err(async_graphql::Error::new("File not found")),
        }
    }

    async fn file_references(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> Result<Vec<super::types::file::FileReference>> {
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
            .map(|(content_id, collection_name)| super::types::file::FileReference {
                content_id,
                collection_name,
                field_name: String::new(),
            })
            .collect())
    }
}
