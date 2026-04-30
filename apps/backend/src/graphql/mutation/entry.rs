use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;
use crate::graphql::types::entry::*;

pub struct EntryMutation;

#[Object]
impl EntryMutation {
    pub async fn create_entry(&self, ctx: &Context<'_>, input: CreateEntryInput) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let entry = gql_ctx
            .services
            .entry
            .create_entry(site_id, &input.collection_id, &input.data.0, &input.slug)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(db_entry_to_gql(entry))
    }

    pub async fn update_entry(&self, ctx: &Context<'_>, id: String, input: UpdateEntryInput) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let entry = gql_ctx
            .services
            .entry
            .update_entry(
                &id,
                site_id,
                input.data.as_ref().map(|d| &d.0),
                input.slug.as_deref(),
                input.status.as_deref(),
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(db_entry_to_gql(entry))
    }

    pub async fn delete_entry(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        gql_ctx
            .services
            .entry
            .delete_entry(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(true)
    }

    pub async fn publish_entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let entry = gql_ctx
            .services
            .entry
            .publish_entry(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(db_entry_to_gql(entry))
    }

    pub async fn unpublish_entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let entry = gql_ctx
            .services
            .entry
            .unpublish_entry(&id, site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(db_entry_to_gql(entry))
    }
}
