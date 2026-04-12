use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::site::Site;
use crate::middleware::auth::SCOPE_SITES_READ;

pub struct AdminQueryRoot;

#[Object]
impl AdminQueryRoot {
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
}
