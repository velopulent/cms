use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::graphql::context::GqlContext;
use crate::graphql::types::content::*;

pub struct ContentMutation;

#[Object]
impl ContentMutation {
    pub async fn create_content(
        &self,
        ctx: &Context<'_>,
        input: CreateContentInput,
    ) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let data_str = input.data.to_string();
        let id = Uuid::now_v7().to_string();

        let result = sqlx::query(
            "INSERT INTO content (id, site_id, collection_id, data, slug) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(site_id)
        .bind(&input.collection_id)
        .bind(&data_str)
        .bind(&input.slug)
        .execute(&gql_ctx.pool)
        .await;

        match result {
            Ok(_) => {
                let db_content = sqlx::query_as::<_, crate::models::content::Content>(
                    "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
                )
                .bind(&id)
                .fetch_one(&gql_ctx.pool)
                .await
                .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

                Ok(db_content_to_gql(db_content))
            }
            Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => {
                Err(async_graphql::Error::new(
                    "Content with this slug already exists for this collection",
                ))
            }
            Err(e) => Err(async_graphql::Error::new(format!("Database error: {}", e))),
        }
    }

    pub async fn update_content(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateContentInput,
    ) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let existing = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ? AND site_id = ?",
        )
        .bind(&id)
        .bind(site_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
        .ok_or_else(|| async_graphql::Error::new("Content not found"))?;

        let resolved_data = match input.data {
            Some(d) => d.0,
            None => serde_json::from_str(&existing.data).unwrap_or(serde_json::Value::Null),
        };
        let data_str = resolved_data.to_string();
        let slug = input.slug.unwrap_or(existing.slug);
        let status = input.status.unwrap_or(existing.status);

        let _ = sqlx::query(
            "UPDATE content SET data = ?, slug = ?, status = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(&data_str)
        .bind(&slug)
        .bind(&status)
        .bind(&id)
        .execute(&gql_ctx.pool)
        .await;

        let db_content = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }

    pub async fn delete_content(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let _ = sqlx::query("DELETE FROM content WHERE id = ? AND site_id = ?")
            .bind(&id)
            .bind(site_id)
            .execute(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    pub async fn publish_content(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let result = sqlx::query(
            "UPDATE content SET status = 'published', published_at = datetime('now'), updated_at = datetime('now') WHERE id = ? AND site_id = ?",
        )
        .bind(&id)
        .bind(site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("Content not found"));
        }

        let db_content = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }

    pub async fn unpublish_content(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let result = sqlx::query(
            "UPDATE content SET status = 'draft', updated_at = datetime('now') WHERE id = ? AND site_id = ?",
        )
        .bind(&id)
        .bind(site_id)
        .execute(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(async_graphql::Error::new("Content not found"));
        }

        let db_content = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }
}
