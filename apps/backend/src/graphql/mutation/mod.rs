pub mod collection;
pub mod entry;
pub mod file;
pub mod webhook;

use async_graphql::{Context, Object, Result};
use std::collections::HashMap;

use crate::graphql::context::GqlContext;
use crate::graphql::types::collection::*;
use crate::graphql::types::entry::{CreateEntryInput, Entry, UpdateEntryInput};
use crate::graphql::types::webhook::{db_delivery_to_gql, db_webhook_to_gql};

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn create_collection(&self, ctx: &Context<'_>, input: CreateCollectionInput) -> Result<Collection> {
        collection::CollectionMutation.create_collection(ctx, input).await
    }

    async fn update_collection(
        &self,
        ctx: &Context<'_>,
        slug: String,
        input: UpdateCollectionInput,
    ) -> Result<Collection> {
        collection::CollectionMutation.update_collection(ctx, slug, input).await
    }

    async fn delete_collection(&self, ctx: &Context<'_>, slug: String) -> Result<bool> {
        collection::CollectionMutation.delete_collection(ctx, slug).await
    }

    async fn create_entry(&self, ctx: &Context<'_>, input: CreateEntryInput) -> Result<Entry> {
        entry::EntryMutation.create_entry(ctx, input).await
    }

    async fn update_entry(&self, ctx: &Context<'_>, id: String, input: UpdateEntryInput) -> Result<Entry> {
        entry::EntryMutation.update_entry(ctx, id, input).await
    }

    async fn delete_entry(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        entry::EntryMutation.delete_entry(ctx, id).await
    }

    async fn publish_entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        entry::EntryMutation.publish_entry(ctx, id).await
    }

    async fn unpublish_entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        entry::EntryMutation.unpublish_entry(ctx, id).await
    }

    async fn restore_revision(&self, ctx: &Context<'_>, entry_id: String, revision_number: i64) -> Result<Entry> {
        entry::EntryMutation
            .restore_revision(ctx, entry_id, revision_number)
            .await
    }

    async fn delete_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        file::FileMutation.delete_file(ctx, id).await
    }

    async fn restore_file(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        file::FileMutation.restore_file(ctx, id).await
    }

    async fn batch_delete_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        file::FileMutation.batch_delete_files(ctx, ids).await
    }

    async fn batch_restore_files(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<i64> {
        file::FileMutation.batch_restore_files(ctx, ids).await
    }

    async fn create_webhook(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        label: String,
        url: String,
        headers: Option<String>,
    ) -> Result<crate::graphql::types::webhook::SiteWebhook> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_match(&site_id)?;
        gql_ctx.require_write()?;

        let parsed_headers: HashMap<String, String> = match headers {
            Some(ref h) if !h.is_empty() => serde_json::from_str(h).unwrap_or_default(),
            _ => HashMap::new(),
        };

        let webhook = gql_ctx
            .services
            .webhook
            .create_webhook(&site_id, &label, &url, &parsed_headers, None)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        let decrypted = gql_ctx.services.webhook.decrypt_webhook_headers(&webhook);
        Ok(db_webhook_to_gql(webhook, decrypted))
    }

    async fn update_webhook(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        webhook_id: String,
        label: Option<String>,
        url: Option<String>,
        headers: Option<String>,
    ) -> Result<crate::graphql::types::webhook::SiteWebhook> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_match(&site_id)?;
        gql_ctx.require_write()?;

        let parsed_headers: Option<HashMap<String, String>> =
            headers.map(|h| serde_json::from_str(&h).unwrap_or_default());

        let webhook = gql_ctx
            .services
            .webhook
            .update_webhook(
                &webhook_id,
                &site_id,
                label.as_deref(),
                url.as_deref(),
                parsed_headers.as_ref(),
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        let decrypted = gql_ctx.services.webhook.decrypt_webhook_headers(&webhook);
        Ok(db_webhook_to_gql(webhook, decrypted))
    }

    async fn delete_webhook(&self, ctx: &Context<'_>, site_id: String, webhook_id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_match(&site_id)?;
        gql_ctx.require_write()?;

        let deleted = gql_ctx
            .services
            .webhook
            .delete_webhook(&webhook_id, &site_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(deleted > 0)
    }

    async fn trigger_webhook(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        webhook_id: String,
    ) -> Result<crate::graphql::types::webhook::WebhookDelivery> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_match(&site_id)?;
        gql_ctx.require_write()?;

        let delivery = gql_ctx
            .services
            .webhook
            .trigger_webhook(&webhook_id, &site_id, None)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(db_delivery_to_gql(delivery))
    }
}
