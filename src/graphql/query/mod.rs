use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::collection::Collection;
use super::types::content::Content;
use super::types::file::File;
use super::types::site::Site;

use crate::repository::collection as collection_repo;
use crate::repository::content as content_repo;
use crate::repository::file as file_repo;
use crate::repository::site as site_repo;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn site(&self, ctx: &Context<'_>) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_site = site_repo::get_by_id(&gql_ctx.pool, site_id)
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

        let db_collections = collection_repo::list(&gql_ctx.pool, site_id)
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

        let db_collection = collection_repo::get_by_slug(&gql_ctx.pool, site_id, &slug)
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

        let status_filter = status.as_deref();

        let params = content_repo::ListContentParams {
            site_id,
            collection_slug: r#type.as_deref(),
            collection_id: collection_id.as_deref(),
            status: status_filter,
            search: search.as_deref(),
            published_only: status_filter.is_none(),
        };

        let items = content_repo::list(&gql_ctx.pool, params)
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

        let content = content_repo::get_by_id(&gql_ctx.pool, &id, site_id, false)
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

        let params = file_repo::ListFilesParams {
            site_id,
            trashed: false,
            search: search.as_deref(),
            file_type: file_type.as_deref(),
            page,
            per_page,
        };

        let result = file_repo::list(&gql_ctx.pool, params)
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

        let db_file = file_repo::get_by_id(&gql_ctx.pool, &id, site_id)
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

        let refs = file_repo::get_references_for_site(&gql_ctx.pool, &file_id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(refs
            .into_iter()
            .map(|r| super::types::file::FileReference {
                content_id: r.content_id,
                collection_name: r.collection_name,
                field_name: r.field_name,
            })
            .collect())
    }
}
