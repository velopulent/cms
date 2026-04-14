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
    async fn sites(&self, ctx: &Context<'_>) -> Result<Vec<Site>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_READ)?;

        let sites = gql_ctx
            .services
            .site
            .list_sites_instance()
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(sites
            .into_iter()
            .map(|s| {
                let obj = s.as_object().unwrap();
                Site {
                    id: obj["id"].as_str().unwrap_or("").to_string(),
                    name: obj["name"].as_str().unwrap_or("").to_string(),
                    default_storage_provider: obj["default_storage_provider"].as_str().unwrap_or("").to_string(),
                    created_by: obj["created_by"].as_str().unwrap_or("").to_string(),
                    created_at: obj["created_at"].as_str().unwrap_or("").to_string(),
                    updated_at: obj["updated_at"].as_str().unwrap_or("").to_string(),
                }
            })
            .collect())
    }

    async fn site(&self, ctx: &Context<'_>, id: String) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_READ)?;

        let site = gql_ctx
            .services
            .site
            .get_site(&id)
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

    async fn current_site(&self, ctx: &Context<'_>) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let site = gql_ctx
            .services
            .site
            .get_site(site_id)
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

    async fn collections(&self, ctx: &Context<'_>) -> Result<Vec<Collection>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_collections = gql_ctx
            .services
            .collection
            .list_collections(site_id)
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

        let db_collection = gql_ctx
            .services
            .collection
            .get_collection(site_id, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Collection not found"))?;

        Ok(super::types::collection::db_collection_to_gql(db_collection))
    }

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

        let page_val = page.unwrap_or(1).max(1);
        let per_page_val = per_page.unwrap_or(50).clamp(1, 200);

        let params = ListEntriesParams {
            site_id,
            collection_slug: r#type.as_deref(),
            collection_id: collection_id.as_deref(),
            status: status.as_deref(),
            search: search.as_deref(),
            published_only: status.is_none(),
            page: page_val,
            per_page: per_page_val,
        };

        let result = gql_ctx
            .services
            .entry
            .list_entries(params)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result
            .items
            .into_iter()
            .map(super::types::entry::db_entry_to_gql)
            .collect())
    }

    async fn entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let entry = gql_ctx
            .services
            .entry
            .get_entry(&id, site_id, false)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        Ok(super::types::entry::db_entry_to_gql(entry))
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

        let page_val = page.unwrap_or(1).max(1);
        let per_page: i64 = 30;

        let params = ListFilesParams {
            site_id,
            trashed: false,
            search: search.as_deref(),
            file_type: file_type.as_deref(),
            page: page_val,
            per_page,
        };

        let result = gql_ctx
            .services
            .file
            .list_files(params)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result
            .items
            .into_iter()
            .map(|f| super::types::file::db_file_to_gql(f, gql_ctx))
            .collect())
    }

    async fn file(&self, ctx: &Context<'_>, id: String) -> Result<File> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_file = gql_ctx
            .services
            .file
            .get_file(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("File not found"))?;

        Ok(super::types::file::db_file_to_gql(db_file, gql_ctx))
    }

    async fn file_references(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> Result<Vec<super::types::file::FileReference>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let refs = gql_ctx
            .services
            .file
            .get_file_references(&file_id, site_id)
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
