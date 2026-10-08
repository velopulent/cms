use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::collection::Collection;
use super::types::entry::Entry;
use super::types::file::File;
use super::types::site::Site;
use crate::repository::traits::{ListEntriesParams, ListFilesParams};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn sites(&self, ctx: &Context<'_>) -> Result<Vec<Site>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        gql_ctx.require_token_scope(crate::models::authorization::Action::SiteRead)?;
        let sites = gql_ctx
            .services
            .site
            .list_sites_for_actor(
                gql_ctx
                    .actor
                    .as_ref()
                    .ok_or_else(|| async_graphql::Error::new("Authentication required"))?,
            )
            .await
            .map_err(|e| crate::graphql::service_error("query.sites", e))?;
        Ok(sites
            .into_iter()
            .filter_map(|site| {
                Some(Site {
                    id: site.get("id")?.as_str()?.to_owned(),
                    name: site.get("name")?.as_str()?.to_owned(),
                    storage_provider: site
                        .get("storage_provider")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    created_by: site
                        .get("created_by")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    created_at: site.get("created_at")?.as_str()?.to_owned(),
                    updated_at: site.get("updated_at")?.as_str()?.to_owned(),
                })
            })
            .collect())
    }

    async fn site(&self, ctx: &Context<'_>, id: String) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let actor = gql_ctx
            .actor
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Authentication required"))?;
        crate::services::authorization::AuthorizationService::new(gql_ctx.repository.user.clone())
            .require_site_action(actor, &id, crate::models::authorization::Action::SiteRead)
            .await
            .map_err(|error| crate::graphql::service_error("authorization", error))?;
        let site = gql_ctx
            .services
            .site
            .get_site(&id)
            .await
            .map_err(|e| crate::graphql::service_error("query.site", e))?
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

    async fn current_site(&self, ctx: &Context<'_>) -> Result<Site> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx
            .require_read(crate::models::authorization::Action::SiteRead)
            .await?;

        let site = gql_ctx
            .services
            .site
            .get_site(site_id)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?
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
        gql_ctx
            .require_read(crate::models::authorization::Action::SchemaRead)
            .await?;

        let db_collections = gql_ctx
            .services
            .collection
            .list_collections(site_id)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?;

        Ok(db_collections
            .into_iter()
            .map(super::types::collection::db_collection_to_gql)
            .collect())
    }

    async fn collection(&self, ctx: &Context<'_>, slug: String) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx
            .require_read(crate::models::authorization::Action::SchemaRead)
            .await?;

        let db_collection = gql_ctx
            .services
            .collection
            .get_collection(site_id, &slug)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Collection not found"))?;

        Ok(super::types::collection::db_collection_to_gql(db_collection))
    }

    // Legacy flat queries retain their public argument contract. New clients
    // can use the explicit site namespace; management remains dashboard-only.
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
        include_drafts: Option<bool>,
    ) -> Result<Vec<Entry>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let include_drafts = include_drafts.unwrap_or_else(|| status.as_deref() == Some("draft"));
        gql_ctx.require_content_read(include_drafts).await?;

        let page_val = page.unwrap_or(1).max(1);
        let per_page_val = per_page.unwrap_or(50).clamp(1, 200);

        let params = ListEntriesParams {
            site_id,
            collection_slug: r#type.as_deref(),
            collection_id: collection_id.as_deref(),
            status: status.as_deref(),
            search: search.as_deref(),
            published_only: !include_drafts,
            page: page_val,
            per_page: per_page_val,
        };

        let result = gql_ctx
            .services
            .entry
            .list_entries(params)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?;

        Ok(result
            .items
            .into_iter()
            .map(super::types::entry::db_entry_to_gql)
            .collect())
    }

    async fn entry(&self, ctx: &Context<'_>, id: String, include_drafts: Option<bool>) -> Result<Entry> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        let include_drafts = include_drafts.unwrap_or(false);
        gql_ctx.require_content_read(include_drafts).await?;

        let entry = gql_ctx
            .services
            .entry
            .get_entry(&id, site_id, !include_drafts)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?
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
        gql_ctx
            .require_read(crate::models::authorization::Action::FilesRead)
            .await?;

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
            .map_err(|e| crate::graphql::service_error("query", e))?;

        Ok(result
            .items
            .into_iter()
            .map(|f| super::types::file::db_file_to_gql(f, gql_ctx))
            .collect())
    }

    async fn file(&self, ctx: &Context<'_>, id: String) -> Result<File> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx
            .require_read(crate::models::authorization::Action::FilesRead)
            .await?;

        let db_file = gql_ctx
            .services
            .file
            .get_file(&id, site_id)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?
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
        gql_ctx
            .require_read(crate::models::authorization::Action::FilesRead)
            .await?;

        gql_ctx
            .services
            .file
            .get_file(&file_id, site_id)
            .await
            .map_err(|e| crate::graphql::service_error("query.file_references", e))?
            .ok_or_else(|| async_graphql::Error::new("File not found"))?;

        let refs = gql_ctx
            .services
            .file
            .get_file_references(&file_id, site_id)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?;

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
        gql_ctx.require_content_read(true).await?;

        // Verify entry exists and belongs to site
        gql_ctx
            .services
            .entry
            .get_entry(&entry_id, site_id, false)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        let page_val = page.unwrap_or(1).max(1);
        let per_page_val = per_page.unwrap_or(50).clamp(1, 200);

        let result = gql_ctx
            .services
            .entry
            .list_revisions(&entry_id, site_id, page_val, per_page_val)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?;

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
        gql_ctx.require_content_read(true).await?;

        // Verify entry exists and belongs to site
        gql_ctx
            .services
            .entry
            .get_entry(&entry_id, site_id, false)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;

        let revision = gql_ctx
            .services
            .entry
            .get_revision(&entry_id, site_id, revision_number)
            .await
            .map_err(|e| crate::graphql::service_error("query", e))?
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
}
