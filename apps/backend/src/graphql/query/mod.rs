use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::collection::Collection;
use super::types::entry::Entry;
use super::types::file::File;
use super::types::site::Site;
use super::types::webhook::{WebhookDelivery, db_delivery_to_gql, db_webhook_to_gql};

use crate::repository::traits::{ListEntriesParams, ListFilesParams};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn current_site(&self, ctx: &Context<'_>) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let site = gql_ctx
            .services
            .site
            .get_site(site_id)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Site not found"))?;

        Ok(Site {
            id: site.id,
            name: site.name,
            storage_provider: site.storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        })
    }

    async fn collections(&self, ctx: &Context<'_>) -> Result<Vec<Collection>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_collections = gql_ctx
            .services
            .collection
            .list_collections(site_id)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?;

        Ok(db_collections
            .into_iter()
            .map(super::types::collection::db_collection_to_gql)
            .collect())
    }

    async fn collection(&self, ctx: &Context<'_>, slug: String) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_collection = gql_ctx
            .services
            .collection
            .get_collection(site_id, &slug)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Collection not found"))?;

        Ok(super::types::collection::db_collection_to_gql(db_collection))
    }

    // Each parameter is a GraphQL field argument exposed in the public schema and
    // consumed positionally by the dashboard's queries; collapsing them into an
    // input object would be a breaking schema change, so the arg count stands.
    #[allow(clippy::too_many_arguments)]
    async fn entries(
        &self,
        ctx: &Context<'_>,
        collection_id: Option<String>,
        status: Option<String>,
        r#type: Option<String>,
        search: Option<String>,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> Result<Vec<Entry>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let page_val = page.unwrap_or(1).max(1);
        let per_page_val = per_page.unwrap_or(50).clamp(1, 200);

        let params = ListEntriesParams {
            site_id,
            collection_slug: r#type.as_deref(),
            collection_id: collection_id.as_deref(),
            status: status.as_deref(),
            search: search.as_deref(),
            published_only: status.is_none(),
            page: page_val,
            per_page: per_page_val,
        };

        let result = gql_ctx
            .services
            .entry
            .list_entries(params)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?;

        Ok(result
            .items
            .into_iter()
            .map(super::types::entry::db_entry_to_gql)
            .collect())
    }

    async fn entry(&self, ctx: &Context<'_>, id: String) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let entry = gql_ctx
            .services
            .entry
            .get_entry(&id, site_id, false)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        Ok(super::types::entry::db_entry_to_gql(entry))
    }

    async fn files(
        &self,
        ctx: &Context<'_>,
        page: Option<i64>,
        search: Option<String>,
        file_type: Option<String>,
    ) -> Result<Vec<File>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let page_val = page.unwrap_or(1).max(1);
        let per_page: i64 = 30;

        let params = ListFilesParams {
            site_id,
            trashed: false,
            search: search.as_deref(),
            file_type: file_type.as_deref(),
            page: page_val,
            per_page,
        };

        let result = gql_ctx
            .services
            .file
            .list_files(params)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?;

        Ok(result
            .items
            .into_iter()
            .map(|f| super::types::file::db_file_to_gql(f, gql_ctx))
            .collect())
    }

    async fn file(&self, ctx: &Context<'_>, id: String) -> Result<File> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let db_file = gql_ctx
            .services
            .file
            .get_file(&id, site_id)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("File not found"))?;

        Ok(super::types::file::db_file_to_gql(db_file, gql_ctx))
    }

    async fn file_references(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> Result<Vec<super::types::file::FileReference>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let refs = gql_ctx
            .services
            .file
            .get_file_references(&file_id, site_id)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?;

        Ok(refs
            .into_iter()
            .map(|r| super::types::file::FileReference {
                entry_id: r.entry_id,
                collection_name: r.collection_name,
                field_name: r.field_name,
            })
            .collect())
    }

    async fn entry_revisions(
        &self,
        ctx: &Context<'_>,
        entry_id: String,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> Result<super::types::entry::RevisionsListResult> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        // Verify entry exists and belongs to site
        gql_ctx
            .services
            .entry
            .get_entry(&entry_id, site_id, false)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        let page_val = page.unwrap_or(1).max(1);
        let per_page_val = per_page.unwrap_or(50).clamp(1, 200);

        let result = gql_ctx
            .services
            .entry
            .list_revisions(&entry_id, site_id, page_val, per_page_val)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?;

        Ok(super::types::entry::RevisionsListResult {
            items: result
                .items
                .into_iter()
                .map(|r| super::types::entry::db_revision_to_gql(r, None))
                .collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        })
    }

    async fn entry_revision(
        &self,
        ctx: &Context<'_>,
        entry_id: String,
        revision_number: i64,
        diff: Option<bool>,
    ) -> Result<super::types::entry::EntryRevision> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        // Verify entry exists and belongs to site
        gql_ctx
            .services
            .entry
            .get_entry(&entry_id, site_id, false)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        let revision = gql_ctx
            .services
            .entry
            .get_revision(&entry_id, site_id, revision_number)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Revision not found"))?;

        let diff_value = if diff.unwrap_or(false) && revision_number > 1 {
            if let Ok(Some(prev)) = gql_ctx
                .services
                .entry
                .get_revision(&entry_id, site_id, revision_number - 1)
                .await
            {
                crate::utils::diff::compute_diff_for_revision(&revision, Some(&prev))
            } else {
                None
            }
        } else {
            None
        };

        Ok(super::types::entry::db_revision_to_gql(revision, diff_value))
    }

    async fn webhooks(&self, ctx: &Context<'_>, site_id: String) -> Result<Vec<super::types::webhook::SiteWebhook>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_match(&site_id)?;
        gql_ctx.require_read()?;

        let webhooks = gql_ctx
            .services
            .webhook
            .list_webhooks(&site_id)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?;

        Ok(webhooks
            .into_iter()
            .map(|w| {
                let headers = gql_ctx.services.webhook.decrypt_webhook_headers(&w);
                db_webhook_to_gql(w, headers)
            })
            .collect())
    }

    async fn webhook(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        webhook_id: String,
    ) -> Result<super::types::webhook::SiteWebhook> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_match(&site_id)?;
        gql_ctx.require_read()?;

        let webhook = gql_ctx
            .services
            .webhook
            .get_webhook(&webhook_id, &site_id)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Webhook not found"))?;

        let headers = gql_ctx.services.webhook.decrypt_webhook_headers(&webhook);
        Ok(db_webhook_to_gql(webhook, headers))
    }

    async fn webhook_deliveries(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        webhook_id: String,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> Result<Vec<WebhookDelivery>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_site_match(&site_id)?;
        gql_ctx.require_read()?;

        let page_val = page.unwrap_or(1).max(1);
        let per_page_val = per_page.unwrap_or(20).clamp(1, 100);

        let (deliveries, _total) = gql_ctx
            .services
            .webhook
            .list_deliveries(&webhook_id, &site_id, page_val, per_page_val)
            .await
            .map_err(|e| crate::graphql::internal_error("query", e))?;

        Ok(deliveries.into_iter().map(db_delivery_to_gql).collect())
    }
}
