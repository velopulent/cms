use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::collection::*;
use super::types::content::*;
use super::types::file::*;
use super::types::site::Site;

pub struct CollectionQuery;

#[Object]
impl CollectionQuery {
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

        Ok(db_collections.into_iter().map(db_collection_to_gql).collect())
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
            Some(c) => Ok(db_collection_to_gql(c)),
            None => Err(async_graphql::Error::new("Collection not found")),
        }
    }
}
