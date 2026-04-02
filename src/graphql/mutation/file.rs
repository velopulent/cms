use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;
use crate::repository::file as file_repo;

pub struct FileMutation;

#[Object]
impl FileMutation {
    pub async fn delete_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        file_repo::soft_delete(&gql_ctx.pool, &id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    pub async fn restore_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        file_repo::restore(&gql_ctx.pool, &id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    pub async fn batch_delete_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let count = file_repo::batch_soft_delete(&gql_ctx.pool, site_id, &ids)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(count as i64)
    }

    pub async fn batch_restore_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let count = file_repo::batch_restore(&gql_ctx.pool, site_id, &ids)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(count as i64)
    }
}
