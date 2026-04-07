use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::graphql::context::GqlContext;
use crate::graphql::types::content::*;
use crate::repository::error::RepositoryError;

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
        gql_ctx.require_write()?;

        let data_str = input.data.to_string();
        let id = Uuid::now_v7().to_string();

        match gql_ctx.repository.content.create(
            &id,
            site_id,
            &input.collection_id,
            &data_str,
            &input.slug,
        )
        .await
        {
            Ok(db_content) => Ok(db_content_to_gql(db_content)),
            Err(RepositoryError::UniqueViolation(_)) => {
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
        gql_ctx.require_write()?;

        let existing = gql_ctx.repository.content.get_by_id(&id, site_id, false)
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

        let db_content = gql_ctx.repository.content.update(&id, &data_str, &slug, &status)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }

    pub async fn delete_content(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        gql_ctx.repository.content.delete(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    pub async fn publish_content(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let db_content = gql_ctx.repository.content.publish(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }

    pub async fn unpublish_content(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let db_content = gql_ctx.repository.content.unpublish(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_content_to_gql(db_content))
    }
}
