use async_graphql::{ComplexObject, SimpleObject};

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub storage_provider: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[ComplexObject]
impl Site {
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

    #[allow(clippy::too_many_arguments)]
    async fn entries(
        &self,
        ctx: &async_graphql::Context<'_>,
        collection_slug: Option<String>,
        include_drafts: Option<bool>,
        filter: Option<super::entry::EntryFilterInput>,
        order_by: Option<super::entry::EntryOrderInput>,
        search: Option<String>,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> async_graphql::Result<super::entry::EntryConnection> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        let include_drafts = include_drafts.unwrap_or(false);
        gql_ctx
            .require_site_action_for(
                &self.id,
                if include_drafts {
                    crate::models::authorization::Action::ContentPreviewRead
                } else {
                    crate::models::authorization::Action::ContentRead
                },
            )
            .await?;
        let filter = filter.unwrap_or_default();
        let search = search.or(filter.search);
        let status = filter.status;
        // Do not silently acknowledge filters or ordering that the shared query
        // service cannot execute. Clients must receive an actionable error.
        if filter.slug.is_some() {
            return Err(async_graphql::Error::new("Slug filtering is not supported"));
        }
        if order_by.is_some() {
            return Err(async_graphql::Error::new("Custom entry ordering is not supported"));
        }
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
        let page_number = result.page;
        let per_page = result.per_page;
        let total = result.total;
        let start_cursor = result.items.first().map(|entry| entry.id.clone());
        let end_cursor = result.items.last().map(|entry| entry.id.clone());
        let nodes = result
            .items
            .into_iter()
            .map(super::entry::db_entry_to_gql)
            .collect::<Vec<_>>();
        Ok(super::entry::EntryConnection {
            nodes,
            page_info: super::entry::PageInfo {
                has_next_page: page_number.saturating_mul(per_page) < total,
                has_previous_page: page_number > 1,
                start_cursor,
                end_cursor,
            },
            total_count: total,
        })
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
}
