use async_graphql::{Context, Object, Result};

use super::context::{GqlAuth, GqlContext};
use super::types::*;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    // --- Auth ---

    async fn me(&self, ctx: &Context<'_>) -> Result<UserPublic> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let user_id = gql_ctx.require_jwt()?;

        let user: Option<(String, String, String)> = sqlx::query_as(
            "SELECT id, username, email FROM users WHERE id = ?",
        )
        .bind(user_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match user {
            Some((id, username, email)) => Ok(UserPublic { id, username, email }),
            None => Err(async_graphql::Error::new("User not found")),
        }
    }

    // --- Sites ---

    async fn sites(&self, ctx: &Context<'_>) -> Result<Vec<SiteWithRole>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let user_id = gql_ctx.require_jwt()?;

        let sites = sqlx::query_as::<_, crate::models::site::SiteWithRole>(
            "SELECT s.id, s.name, s.default_storage_provider, s.created_by, s.created_at, s.updated_at, sm.role
             FROM sites s
             JOIN site_members sm ON s.id = sm.site_id
             WHERE sm.user_id = ?
             ORDER BY s.name",
        )
        .bind(user_id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(sites
            .into_iter()
            .map(|s| SiteWithRole {
                id: s.id,
                name: s.name,
                default_storage_provider: s.default_storage_provider,
                created_by: s.created_by,
                created_at: s.created_at,
                updated_at: s.updated_at,
                role: s.role,
            })
            .collect())
    }

    async fn site(&self, ctx: &Context<'_>, id: String) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&id, "viewer").await?;

        let db_site = sqlx::query_as::<_, crate::models::site::Site>(
            "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
        )
        .bind(&id)
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

    // --- Members ---

    async fn members(&self, ctx: &Context<'_>, site_id: String) -> Result<Vec<SiteMember>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "viewer").await?;

        let db_members = sqlx::query_as::<_, crate::models::site::SiteMember>(
            "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
             FROM site_members sm
             JOIN users u ON sm.user_id = u.id
             WHERE sm.site_id = ?
             ORDER BY sm.role DESC, u.username",
        )
        .bind(&site_id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_members
            .into_iter()
            .map(|m| SiteMember {
                id: m.id,
                site_id: m.site_id,
                user_id: m.user_id,
                username: m.username,
                email: m.email,
                role: m.role,
                created_at: m.created_at,
            })
            .collect())
    }

    // --- Collections ---

    async fn collections(&self, ctx: &Context<'_>, site_id: String) -> Result<Vec<Collection>> {
        let gql_ctx = ctx.data::<GqlContext>()?;

        match &gql_ctx.auth {
            Some(GqlAuth::Jwt { .. }) => {
                gql_ctx.require_site_access(&site_id, "viewer").await?;
            }
            Some(GqlAuth::ApiKey { .. }) => {
                gql_ctx.require_api_key_site(&site_id)?;
            }
            None => return Err(async_graphql::Error::new("Authentication required")),
        }

        let db_collections = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? ORDER BY name",
        )
        .bind(&site_id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_collections.into_iter().map(db_collection_to_gql).collect())
    }

    async fn collection(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        slug: String,
    ) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;

        match &gql_ctx.auth {
            Some(GqlAuth::Jwt { .. }) => {
                gql_ctx.require_site_access(&site_id, "viewer").await?;
            }
            Some(GqlAuth::ApiKey { .. }) => {
                gql_ctx.require_api_key_site(&site_id)?;
            }
            None => return Err(async_graphql::Error::new("Authentication required")),
        }

        let db_collection = sqlx::query_as::<_, crate::models::collection::Collection>(
            "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
        )
        .bind(&site_id)
        .bind(&slug)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match db_collection {
            Some(c) => Ok(db_collection_to_gql(c)),
            None => Err(async_graphql::Error::new("Collection not found")),
        }
    }

    // --- Content ---

    async fn content(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        collection_id: Option<String>,
        status: Option<String>,
        r#type: Option<String>,
        search: Option<String>,
    ) -> Result<Vec<Content>> {
        let gql_ctx = ctx.data::<GqlContext>()?;

        let is_api_key = matches!(gql_ctx.auth, Some(GqlAuth::ApiKey { .. }));

        match &gql_ctx.auth {
            Some(GqlAuth::Jwt { .. }) => {
                gql_ctx.require_site_access(&site_id, "viewer").await?;
            }
            Some(GqlAuth::ApiKey { .. }) => {
                gql_ctx.require_api_key_site(&site_id)?;
            }
            None => return Err(async_graphql::Error::new("Authentication required")),
        }

        let mut query = String::from(
            "SELECT c.id, c.site_id, c.collection_id, c.data, c.slug, c.status, c.created_at, c.updated_at, c.published_at
             FROM content c
             JOIN collections col ON c.collection_id = col.id
             WHERE c.site_id = ?",
        );
        let mut bindings: Vec<String> = vec![site_id];

        if is_api_key {
            query.push_str(" AND c.status = 'published'");
        }

        if let Some(cid) = collection_id {
            query.push_str(" AND c.collection_id = ?");
            bindings.push(cid);
        }

        if let Some(content_type) = &r#type {
            query.push_str(" AND col.slug = ?");
            bindings.push(content_type.clone());
        }

        if let Some(s) = &status {
            if !is_api_key {
                query.push_str(" AND c.status = ?");
                bindings.push(s.clone());
            }
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

        Ok(items.into_iter().map(db_content_to_gql).collect())
    }

    async fn content_item(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
    ) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;

        let is_api_key = matches!(gql_ctx.auth, Some(GqlAuth::ApiKey { .. }));

        match &gql_ctx.auth {
            Some(GqlAuth::Jwt { .. }) => {
                gql_ctx.require_site_access(&site_id, "viewer").await?;
            }
            Some(GqlAuth::ApiKey { .. }) => {
                gql_ctx.require_api_key_site(&site_id)?;
            }
            None => return Err(async_graphql::Error::new("Authentication required")),
        }

        let mut query = String::from(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ? AND site_id = ?",
        );

        if is_api_key {
            query.push_str(" AND status = 'published'");
        }

        let content = sqlx::query_as::<_, crate::models::content::Content>(&query)
            .bind(&id)
            .bind(&site_id)
            .fetch_optional(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match content {
            Some(c) => Ok(db_content_to_gql(c)),
            None => Err(async_graphql::Error::new("Content not found")),
        }
    }

    // --- Files ---

    async fn files(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        page: Option<i64>,
        search: Option<String>,
        file_type: Option<String>,
    ) -> Result<Vec<File>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "viewer").await?;

        let page = page.unwrap_or(1).max(1);
        let per_page: i64 = 30;
        let offset = (page - 1) * per_page;

        let mut query = String::from(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at
             FROM files WHERE site_id = ? AND deleted_at IS NULL",
        );
        let mut bindings: Vec<String> = vec![site_id];

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

    async fn file(&self, ctx: &Context<'_>, site_id: String, id: String) -> Result<File> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "viewer").await?;

        let db_file = sqlx::query_as::<_, crate::models::file::File>(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at
             FROM files WHERE id = ? AND site_id = ? AND deleted_at IS NULL",
        )
        .bind(&id)
        .bind(&site_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match db_file {
            Some(f) => Ok(db_file_to_gql(f, gql_ctx)),
            None => Err(async_graphql::Error::new("File not found")),
        }
    }

    async fn file_references(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        file_id: String,
    ) -> Result<Vec<FileReference>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "viewer").await?;

        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT DISTINCT c.id, col.name FROM content_file_references cfr
             JOIN content c ON cfr.content_id = c.id
             JOIN collections col ON c.collection_id = col.id
             WHERE cfr.file_id = ?",
        )
        .bind(&file_id)
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

    // --- API Keys ---

    async fn api_keys(&self, ctx: &Context<'_>, site_id: String) -> Result<Vec<ApiKey>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_access(&site_id, "admin").await?;

        let db_keys = sqlx::query_as::<_, crate::models::api_key::ApiKey>(
            "SELECT id, site_id, name, key_prefix, permissions, last_used_at, created_at, expires_at
             FROM api_keys WHERE site_id = ? ORDER BY created_at DESC",
        )
        .bind(&site_id)
        .fetch_all(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_keys
            .into_iter()
            .map(|k| ApiKey {
                id: k.id,
                site_id: k.site_id,
                name: k.name,
                key_prefix: k.key_prefix,
                permissions: k.permissions,
                last_used_at: k.last_used_at,
                created_at: k.created_at,
                expires_at: k.expires_at,
            })
            .collect())
    }
}
