use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::graphql::context::GqlContext;
use crate::graphql::types::entry::*;
use crate::repository::error::RepositoryError;

pub struct EntryMutation;

#[Object]
impl EntryMutation {
    pub async fn create_entry(&self, ctx: &Context<'_>, input: CreateEntryInput) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let data_str = input.data.to_string();
        let id = Uuid::now_v7().to_string();

        match gql_ctx
            .repository
            .entry
            .create(&id, site_id, &input.collection_id, &data_str, &input.slug)
            .await
        {
            Ok(db_entry) => Ok(db_entry_to_gql(db_entry)),
            Err(RepositoryError::UniqueViolation(_)) => Err(async_graphql::Error::new(
                "Entry with this slug already exists for this collection",
            )),
            Err(e) => Err(async_graphql::Error::new(format!("Database error: {}", e))),
        }
    }

    pub async fn update_entry(&self, ctx: &Context<'_>, id: String, input: UpdateEntryInput) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let existing = gql_ctx
            .repository
            .entry
            .get_by_id(&id, site_id, false)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        let resolved_data = match input.data {
            Some(d) => d.0,
            None => serde_json::from_str(&existing.data).unwrap_or(serde_json::Value::Null),
        };
        let data_str = resolved_data.to_string();
        let slug = input.slug.unwrap_or(existing.slug);
        let status = input.status.unwrap_or(existing.status);

        let db_entry = gql_ctx
            .repository
            .entry
            .update(&id, &data_str, &slug, &status)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_entry_to_gql(db_entry))
    }

    pub async fn delete_entry(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        gql_ctx
            .repository
            .entry
            .delete(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    pub async fn publish_entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let db_entry = gql_ctx
            .repository
            .entry
            .publish(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_entry_to_gql(db_entry))
    }

    pub async fn unpublish_entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let db_entry = gql_ctx
            .repository
            .entry
            .unpublish(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(db_entry_to_gql(db_entry))
    }
}
