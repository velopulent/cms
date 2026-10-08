use async_graphql::{ComplexObject, SimpleObject};

/// Namespace for one site's content. Every field authorizes its own action, so a
/// content-only credential can read entries without `site.read`.
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Site {
    pub id: String,
    /// Preloaded by `sites`, which has already authorized each site.
    #[graphql(skip)]
    pub details: Option<SiteDetails>,
}

#[derive(Clone)]
pub struct SiteDetails {
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[ComplexObject]
impl Site {
    async fn name(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<String> {
        Ok(self.load_details(ctx).await?.name)
    }

    async fn created_at(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<String> {
        Ok(self.load_details(ctx).await?.created_at)
    }

    async fn updated_at(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<String> {
        Ok(self.load_details(ctx).await?.updated_at)
    }

    async fn collections(
        &self,
        ctx: &async_graphql::Context<'_>,
    ) -> async_graphql::Result<Vec<super::collection::Collection>> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(&self.id, crate::models::authorization::Action::SchemaRead)
            .await?;
        let collections = gql_ctx
            .services
            .collection
            .list_collections(&self.id)
            .await
            .map_err(|e| crate::graphql::service_error("site.collections", e))?;
        Ok(collections
            .into_iter()
            .map(super::collection::db_collection_to_gql)
            .collect())
    }

    async fn collection(
        &self,
        ctx: &async_graphql::Context<'_>,
        slug: String,
    ) -> async_graphql::Result<super::collection::Collection> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(&self.id, crate::models::authorization::Action::SchemaRead)
            .await?;
        let collection = gql_ctx
            .services
            .collection
            .get_collection(&self.id, &slug)
            .await
            .map_err(|e| crate::graphql::service_error("site.collection", e))?
            .ok_or_else(|| async_graphql::Error::new("Collection not found"))?;
        Ok(super::collection::db_collection_to_gql(collection))
    }

    #[allow(clippy::too_many_arguments)]
    async fn entries(
        &self,
        ctx: &async_graphql::Context<'_>,
        collection_slug: Option<String>,
        status: Option<String>,
        search: Option<String>,
        include_drafts: Option<bool>,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> async_graphql::Result<super::entry::EntryConnection> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        let include_drafts = include_drafts.unwrap_or(false);
        gql_ctx
            .require_site_action_for(&self.id, content_read_action(include_drafts))
            .await?;
        let result = gql_ctx
            .services
            .entry
            .list_entries(crate::repository::traits::ListEntriesParams {
                site_id: &self.id,
                collection_slug: collection_slug.as_deref(),
                collection_id: None,
                status: status.as_deref(),
                search: search.as_deref(),
                published_only: !include_drafts,
                page: page.unwrap_or(1).max(1),
                per_page: per_page.unwrap_or(50).clamp(1, 200),
            })
            .await
            .map_err(|e| crate::graphql::service_error("site.entries", e))?;
        Ok(super::entry::EntryConnection {
            page_info: super::entry::PageInfo {
                has_next_page: result.page.saturating_mul(result.per_page) < result.total,
                has_previous_page: result.page > 1,
            },
            total_count: result.total,
            nodes: result.items.into_iter().map(super::entry::db_entry_to_gql).collect(),
        })
    }

    async fn entry(
        &self,
        ctx: &async_graphql::Context<'_>,
        id: String,
        include_drafts: Option<bool>,
    ) -> async_graphql::Result<super::entry::Entry> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        let include_drafts = include_drafts.unwrap_or(false);
        gql_ctx
            .require_site_action_for(&self.id, content_read_action(include_drafts))
            .await?;
        let entry = gql_ctx
            .services
            .entry
            .get_entry(&id, &self.id, !include_drafts)
            .await
            .map_err(|e| crate::graphql::service_error("site.entry", e))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;
        Ok(super::entry::db_entry_to_gql(entry))
    }

    /// Revision history; revisions can hold unpublished content, so reading them
    /// requires content.preview.read.
    async fn entry_revisions(
        &self,
        ctx: &async_graphql::Context<'_>,
        entry_id: String,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> async_graphql::Result<super::entry::RevisionsListResult> {
        let gql_ctx = self.require_revision_access(ctx, &entry_id).await?;
        let result = gql_ctx
            .services
            .entry
            .list_revisions(
                &entry_id,
                &self.id,
                page.unwrap_or(1).max(1),
                per_page.unwrap_or(50).clamp(1, 200),
            )
            .await
            .map_err(|e| crate::graphql::service_error("site.entry_revisions", e))?;
        Ok(super::entry::RevisionsListResult {
            items: result
                .items
                .into_iter()
                .map(|revision| super::entry::db_revision_to_gql(revision, None))
                .collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        })
    }

    async fn entry_revision(
        &self,
        ctx: &async_graphql::Context<'_>,
        entry_id: String,
        revision_number: i64,
        diff: Option<bool>,
    ) -> async_graphql::Result<super::entry::EntryRevision> {
        let gql_ctx = self.require_revision_access(ctx, &entry_id).await?;
        let entries = &gql_ctx.services.entry;
        let revision = entries
            .get_revision(&entry_id, &self.id, revision_number)
            .await
            .map_err(|e| crate::graphql::service_error("site.entry_revision", e))?
            .ok_or_else(|| async_graphql::Error::new("Revision not found"))?;
        let diff = if diff.unwrap_or(false) && revision_number > 1 {
            match entries.get_revision(&entry_id, &self.id, revision_number - 1).await {
                Ok(Some(previous)) => crate::utils::diff::compute_diff_for_revision(&revision, Some(&previous)),
                _ => None,
            }
        } else {
            None
        };
        Ok(super::entry::db_revision_to_gql(revision, diff))
    }

    async fn singletons(
        &self,
        ctx: &async_graphql::Context<'_>,
        include_drafts: Option<bool>,
    ) -> async_graphql::Result<Vec<super::collection::SingletonGraphql>> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(
                &self.id,
                if include_drafts.unwrap_or(false) {
                    crate::models::authorization::Action::ContentPreviewRead
                } else {
                    crate::models::authorization::Action::ContentRead
                },
            )
            .await?;
        let items = gql_ctx
            .services
            .singleton
            .list_singletons_visible(&self.id, !include_drafts.unwrap_or(false))
            .await
            .map_err(|e| crate::graphql::service_error("site.singletons", e))?;
        Ok(items.into_iter().map(super::collection::singleton_to_gql).collect())
    }

    async fn files(
        &self,
        ctx: &async_graphql::Context<'_>,
        page: Option<i64>,
        search: Option<String>,
        file_type: Option<String>,
    ) -> async_graphql::Result<Vec<super::file::File>> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(&self.id, crate::models::authorization::Action::FilesRead)
            .await?;
        let result = gql_ctx
            .services
            .file
            .list_files(crate::repository::traits::ListFilesParams {
                site_id: &self.id,
                trashed: false,
                search: search.as_deref(),
                file_type: file_type.as_deref(),
                page: page.unwrap_or(1).max(1),
                per_page: 50,
            })
            .await
            .map_err(|e| crate::graphql::service_error("site.files", e))?;
        Ok(result
            .items
            .into_iter()
            .map(|file| super::file::db_file_to_gql(file, gql_ctx))
            .collect())
    }

    async fn file(&self, ctx: &async_graphql::Context<'_>, id: String) -> async_graphql::Result<super::file::File> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(&self.id, crate::models::authorization::Action::FilesRead)
            .await?;
        let file = gql_ctx
            .services
            .file
            .get_file(&id, &self.id)
            .await
            .map_err(|e| crate::graphql::service_error("site.file", e))?
            .ok_or_else(|| async_graphql::Error::new("File not found"))?;
        Ok(super::file::db_file_to_gql(file, gql_ctx))
    }

    async fn file_references(
        &self,
        ctx: &async_graphql::Context<'_>,
        file_id: String,
    ) -> async_graphql::Result<Vec<super::file::FileReference>> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(&self.id, crate::models::authorization::Action::FilesRead)
            .await?;
        let files = &gql_ctx.services.file;
        files
            .get_file(&file_id, &self.id)
            .await
            .map_err(|e| crate::graphql::service_error("site.file_references", e))?
            .ok_or_else(|| async_graphql::Error::new("File not found"))?;
        Ok(files
            .get_file_references(&file_id, &self.id)
            .await
            .map_err(|e| crate::graphql::service_error("site.file_references", e))?
            .into_iter()
            .map(|reference| super::file::FileReference {
                entry_id: reference.entry_id,
                collection_name: reference.collection_name,
                field_name: reference.field_name,
            })
            .collect())
    }
}

impl Site {
    async fn load_details(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<SiteDetails> {
        if let Some(details) = &self.details {
            return Ok(details.clone());
        }
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(&self.id, crate::models::authorization::Action::SiteRead)
            .await?;
        let site = gql_ctx
            .services
            .site
            .get_site(&self.id)
            .await
            .map_err(|e| crate::graphql::service_error("site", e))?
            .ok_or_else(|| async_graphql::Error::new("Site not found"))?;
        Ok(SiteDetails {
            name: site.name,
            created_at: site.created_at,
            updated_at: site.updated_at,
        })
    }

    async fn require_revision_access<'a>(
        &self,
        ctx: &async_graphql::Context<'a>,
        entry_id: &str,
    ) -> async_graphql::Result<&'a crate::graphql::context::GqlContext> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(&self.id, crate::models::authorization::Action::ContentPreviewRead)
            .await?;
        gql_ctx
            .services
            .entry
            .get_entry(entry_id, &self.id, false)
            .await
            .map_err(|e| crate::graphql::service_error("site.entry", e))?
            .ok_or_else(|| async_graphql::Error::new("Entry not found"))?;
        Ok(gql_ctx)
    }
}

fn content_read_action(include_drafts: bool) -> crate::models::authorization::Action {
    if include_drafts {
        crate::models::authorization::Action::ContentPreviewRead
    } else {
        crate::models::authorization::Action::ContentRead
    }
}
