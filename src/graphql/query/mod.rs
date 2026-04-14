use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::collection::Collection;
use super::types::entry::Entry;
use super::types::file::File;
use super::types::site::Site;

use crate::middleware::auth::SCOPE_SITES_READ;
use crate::repository::traits::{ListEntriesParams, ListFilesParams};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Instance-scoped: List all sites (requires cms_ik_* token with sites:read scope)
    async fn sites(&self, ctx: &Context<'_>) -> Result<Vec<Site>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_READ)?;

        let sites = gql_ctx
            .repository
            .site
            .list_all()
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(sites
            .into_iter()
            .map(|s| Site {
                id: s.id,
                name: s.name,
                default_storage_provider: s.default_storage_provider,
                created_by: s.created_by,
                created_at: s.created_at,
                updated_at: s.updated_at,
            })
            .collect())
    }

    /// Instance-scoped: Get a site by ID (requires cms_ik_* token with sites:read scope)
    async fn site(&self, ctx: &Context<'_>, id: String) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_READ)?;

        let site = gql_ctx
            .repository
            .site
            .get_by_id(&id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Site not found"))?;

        Ok(Site {
            id: site.id,
            name: site.name,
            default_storage_provider: site.default_storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        })
    }

    /// Site-scoped: Get the current site (requires cms_sk_* token)
    async fn current_site(&self, ctx: &Context<'_>) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_site = gql_ctx
            .repository
            .site
            .get_by_id(site_id)
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

    /// Site-scoped: List collections (requires cms_sk_* token)
    async fn collections(&self, ctx: &Context<'_>) -> Result<Vec<Collection>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_collections = gql_ctx
            .repository
            .collection
            .list(site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_collections
            .into_iter()
            .map(super::types::collection::db_collection_to_gql)
            .collect())
    }

    /// Site-scoped: Get a collection by slug (requires cms_sk_* token)
    async fn collection(&self, ctx: &Context<'_>, slug: String) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_collection = gql_ctx
            .repository
            .collection
            .get_by_slug(site_id, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match db_collection {
            Some(c) => Ok(super::types::collection::db_collection_to_gql(c)),
            None => Err(async_graphql::Error::new("Collection not found")),
        }
    }

    /// Site-scoped: List entries (requires cms_sk_* token)
    async fn entries(
        &self,
        ctx: &Context<'_>,
        collection_id: Option<String>,
        status: Option<String>,
        r#type: Option<String>,
        search: Option<String>,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> Result<Vec<Entry>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let status_filter = status.as_deref();
        let page_val = page.unwrap_or(1).max(1);
        let per_page_val = per_page.unwrap_or(50).clamp(1, 200);

        let params = ListEntriesParams {
            site_id,
            collection_slug: r#type.as_deref(),
            collection_id: collection_id.as_deref(),
            status: status_filter,
            search: search.as_deref(),
            published_only: status_filter.is_none(),
            page: page_val,
            per_page: per_page_val,
        };

        let result = gql_ctx
            .repository
            .entry
            .list(params)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result
            .items
            .into_iter()
            .map(super::types::entry::db_entry_to_gql)
            .collect())
    }

    /// Site-scoped: Get an entry by ID (requires cms_sk_* token)
    async fn entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let entry = gql_ctx
            .repository
            .entry
            .get_by_id(&id, site_id, false)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match entry {
            Some(e) => Ok(super::types::entry::db_entry_to_gql(e)),
            None => Err(async_graphql::Error::new("Entry not found")),
        }
    }

    /// Site-scoped: List files (requires cms_sk_* token)
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

        let params = ListFilesParams {
            site_id,
            trashed: false,
            search: search.as_deref(),
            file_type: file_type.as_deref(),
            page,
            per_page,
        };

        let result = gql_ctx
            .repository
            .file
            .list(params)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result
            .items
            .into_iter()
            .map(|f| super::types::file::db_file_to_gql(f, gql_ctx))
            .collect())
    }

    /// Site-scoped: Get a file by ID (requires cms_sk_* token)
    async fn file(&self, ctx: &Context<'_>, id: String) -> Result<File> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_file = gql_ctx
            .repository
            .file
            .get_by_id(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match db_file {
            Some(f) => Ok(super::types::file::db_file_to_gql(f, gql_ctx)),
            None => Err(async_graphql::Error::new("File not found")),
        }
    }

    /// Site-scoped: Get file references (requires cms_sk_* token)
    async fn file_references(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> Result<Vec<super::types::file::FileReference>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let refs = gql_ctx
            .repository
            .file
            .get_references_for_site(&file_id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(refs
            .into_iter()
            .map(|r| super::types::file::FileReference {
                entry_id: r.entry_id,
                collection_name: r.collection_name,
                field_name: r.field_name,
            })
            .collect())
    }
}
