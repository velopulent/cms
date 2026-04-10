pub mod collection;
pub mod entry;
pub mod file;

use async_graphql::{Context, Object, Result};

use crate::graphql::types::collection::*;
use crate::graphql::types::entry::{CreateEntryInput, Entry, UpdateEntryInput};

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // --- Collections ---

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

    // --- Entries ---

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

    // --- Files ---

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
