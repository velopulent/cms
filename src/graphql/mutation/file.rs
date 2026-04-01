use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;

pub struct FileMutation;

#[Object]
impl FileMutation {
    pub async fn delete_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let _ = sqlx::query(
            "UPDATE files SET deleted_at = datetime('now') WHERE id = ? AND site_id = ? AND deleted_at IS NULL",
        )
        .bind(&id)
        .bind(site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    pub async fn restore_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let _ = sqlx::query(
            "UPDATE files SET deleted_at = NULL WHERE id = ? AND site_id = ? AND deleted_at IS NOT NULL",
        )
        .bind(&id)
        .bind(site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    pub async fn batch_delete_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        if ids.is_empty() {
            return Err(async_graphql::Error::new("No file IDs provided"));
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "UPDATE files SET deleted_at = datetime('now') WHERE site_id = ? AND id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut q = sqlx::query(&query).bind(site_id);
        for id in &ids {
            q = q.bind(id);
        }

        let result = q
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result.rows_affected() as i64)
    }

    pub async fn batch_restore_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        if ids.is_empty() {
            return Err(async_graphql::Error::new("No file IDs provided"));
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "UPDATE files SET deleted_at = NULL WHERE site_id = ? AND id IN ({}) AND deleted_at IS NOT NULL",
            placeholders
        );

        let mut q = sqlx::query(&query).bind(site_id);
        for id in &ids {
            q = q.bind(id);
        }

        let result = q
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(result.rows_affected() as i64)
    }
}
