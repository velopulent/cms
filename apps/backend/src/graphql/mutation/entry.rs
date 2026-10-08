use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;
use crate::graphql::types::entry::*;

pub struct EntryMutation;

#[Object]
impl EntryMutation {
    pub async fn create_entry(&self, ctx: &Context<'_>, site_id: String, input: CreateEntryInput) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::ContentWrite)
            .await?;

        let created_by = gql_ctx.user_id();
        let entry = gql_ctx
            .services
            .entry
            .create_entry(&site_id, &input.collection_id, &input.data.0, &input.slug, created_by)
            .await
            .map_err(|e| crate::graphql::service_error("mutation.create_entry", e))?;

        Ok(db_entry_to_gql(entry))
    }

    pub async fn update_entry(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
        input: UpdateEntryInput,
    ) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::ContentWrite)
            .await?;

        if input.status.is_some() {
            gql_ctx
                .require_site_action_for(&site_id, crate::models::authorization::Action::ContentPublish)
                .await?;
        }
        let created_by = gql_ctx.user_id();
        let entry = gql_ctx
            .services
            .entry
            .update_entry(crate::services::entry::UpdateEntryInput {
                id: &id,
                site_id: &site_id,
                data: input.data.as_ref().map(|d| &d.0),
                slug: input.slug.as_deref(),
                status: input.status.as_deref(),
                created_by,
                change_summary: input.change_summary.as_deref(),
                expected_version: input.expected_version.as_deref(),
            })
            .await
            .map_err(|e| crate::graphql::service_error("mutation.update_entry", e))?;

        Ok(db_entry_to_gql(entry))
    }

    pub async fn delete_entry(&self, ctx: &Context<'_>, site_id: String, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::ContentWrite)
            .await?;

        gql_ctx
            .services
            .entry
            .delete_entry(&id, &site_id)
            .await
            .map_err(|e| crate::graphql::service_error("mutation.delete_entry", e))?;

        Ok(true)
    }

    pub async fn set_publication(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
        published: bool,
    ) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::ContentPublish)
            .await?;

        let entry = (if published {
            gql_ctx.services.entry.publish_entry(&id, &site_id).await
        } else {
            gql_ctx.services.entry.unpublish_entry(&id, &site_id).await
        })
        .map_err(|e| crate::graphql::service_error("mutation.publish_entry", e))?;

        Ok(db_entry_to_gql(entry))
    }

    pub async fn restore_revision(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        entry_id: String,
        revision_number: i64,
    ) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx
            .require_site_action_for(&site_id, crate::models::authorization::Action::ContentWrite)
            .await?;

        let created_by = gql_ctx.user_id();
        let entry = gql_ctx
            .services
            .entry
            .restore_revision(&entry_id, &site_id, revision_number, created_by)
            .await
            .map_err(|e| crate::graphql::service_error("mutation.restore_revision", e))?;

        Ok(db_entry_to_gql(entry))
    }
}
