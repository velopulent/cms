use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use super::context::GqlContext;
use super::types::site::Site;
use crate::middleware::auth::{SCOPE_SITES_DELETE, SCOPE_SITES_WRITE};

pub struct AdminMutationRoot;

#[Object]
impl AdminMutationRoot {
    async fn create_site(
        &self,
        ctx: &Context<'_>,
        name: String,
        default_storage_provider: Option<String>,
    ) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_WRITE)?;

        let site = gql_ctx
            .repository
            .site
            .create(
                &Uuid::now_v7().to_string(),
                &name,
                default_storage_provider.as_deref().unwrap_or("filesystem"),
                "system",
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(Site {
            id: site.id,
            name: site.name,
            default_storage_provider: site.default_storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        })
    }

    async fn update_site(
        &self,
        ctx: &Context<'_>,
        id: String,
        name: Option<String>,
        default_storage_provider: Option<String>,
    ) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_WRITE)?;

        let existing = gql_ctx
            .repository
            .site
            .get_by_id(&id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Site not found"))?;

        let site = gql_ctx
            .repository
            .site
            .update(
                &id,
                name.as_deref().unwrap_or(&existing.name),
                default_storage_provider
                    .as_deref()
                    .unwrap_or(&existing.default_storage_provider),
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(Site {
            id: site.id,
            name: site.name,
            default_storage_provider: site.default_storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        })
    }

    async fn delete_site(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_DELETE)?;

        gql_ctx
            .repository
            .site
            .delete(&id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }
}
