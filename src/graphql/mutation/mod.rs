pub mod collection;
pub mod entry;
pub mod file;

use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::graphql::types::collection::*;
use crate::graphql::types::entry::{CreateEntryInput, Entry, UpdateEntryInput};
use crate::graphql::types::site::Site;
use crate::middleware::auth::{SCOPE_SITES_DELETE, SCOPE_SITES_WRITE};

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // --- Instance Mutations (requires InstanceToken) ---

    async fn create_site(
        &self,
        ctx: &Context<'_>,
        name: String,
        default_storage_provider: Option<String>,
    ) -> Result<Site> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_WRITE)?;

        let site = gql_ctx
            .repository
            .site
            .create(
                &Uuid::now_v7().to_string(),
                &name,
                default_storage_provider.as_deref().unwrap_or("filesystem"),
                "system",
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(Site {
            id: site.id,
            name: site.name,
            default_storage_provider: site.default_storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        })
    }

    async fn update_site(
        &self,
        ctx: &Context<'_>,
        id: String,
        name: Option<String>,
        default_storage_provider: Option<String>,
    ) -> Result<Site> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_WRITE)?;

        let existing = gql_ctx
            .repository
            .site
            .get_by_id(&id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?
            .ok_or_else(|| async_graphql::Error::new("Site not found"))?;

        let site = gql_ctx
            .repository
            .site
            .update(
                &id,
                name.as_deref().unwrap_or(&existing.name),
                default_storage_provider
                    .as_deref()
                    .unwrap_or(&existing.default_storage_provider),
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(Site {
            id: site.id,
            name: site.name,
            default_storage_provider: site.default_storage_provider,
            created_by: site.created_by,
            created_at: site.created_at,
            updated_at: site.updated_at,
        })
    }

    async fn delete_site(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx.require_instance_scope(SCOPE_SITES_DELETE)?;

        gql_ctx
            .repository
            .site
            .delete(&id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(true)
    }

    // --- Collections (requires SiteToken) ---

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

    // --- Entries (requires SiteToken) ---

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

    // --- Files (requires SiteToken) ---

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
}
