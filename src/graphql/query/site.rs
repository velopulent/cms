use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::site::Site;

pub struct SiteQuery;

#[Object]
impl SiteQuery {
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
}
