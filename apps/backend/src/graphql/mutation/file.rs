use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;

pub struct FileMutation;

#[Object]
impl FileMutation {
    pub async fn delete_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx
            .require_write(crate::models::authorization::Action::FilesWrite)
            .await?;

        gql_ctx
            .services
            .file
            .soft_delete(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(true)
    }

    pub async fn restore_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx
            .require_write(crate::models::authorization::Action::FilesWrite)
            .await?;

        gql_ctx
            .services
            .file
            .restore(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(true)
    }

    pub async fn batch_delete_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx
            .require_write(crate::models::authorization::Action::FilesWrite)
            .await?;

        let count = gql_ctx
            .services
            .file
            .batch_soft_delete(site_id, &ids)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(count as i64)
    }

    pub async fn batch_restore_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx
            .require_write(crate::models::authorization::Action::FilesWrite)
            .await?;

        let count = gql_ctx
            .services
            .file
            .batch_restore(site_id, &ids)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(count as i64)
    }
}
