pub mod entry;
pub mod file;
pub mod singleton;

use async_graphql::{Context, Object, Result};

use crate::graphql::types::entry::{CreateEntryInput, Entry, UpdateEntryInput};

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn create_entry(&self, ctx: &Context<'_>, site_id: String, input: CreateEntryInput) -> Result<Entry> {
        entry::EntryMutation.create_entry(ctx, site_id, input).await
    }

    async fn update_entry(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
        input: UpdateEntryInput,
    ) -> Result<Entry> {
        entry::EntryMutation.update_entry(ctx, site_id, id, input).await
    }

    async fn delete_entry(&self, ctx: &Context<'_>, site_id: String, id: String) -> Result<bool> {
        entry::EntryMutation.delete_entry(ctx, site_id, id).await
    }

    async fn set_entry_publication(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        id: String,
        published: bool,
    ) -> Result<Entry> {
        entry::EntryMutation.set_publication(ctx, site_id, id, published).await
    }

    async fn restore_revision(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        entry_id: String,
        revision_number: i64,
    ) -> Result<Entry> {
        entry::EntryMutation
            .restore_revision(ctx, site_id, entry_id, revision_number)
            .await
    }

    async fn update_singleton(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        slug: String,
        data: crate::graphql::types::json::Json,
        change_summary: Option<String>,
        expected_version: Option<String>,
    ) -> Result<crate::graphql::types::collection::SingletonGraphql> {
        singleton::SingletonMutation
            .update_singleton(ctx, site_id, slug, data, change_summary, expected_version)
            .await
    }

    async fn delete_file(&self, ctx: &Context<'_>, site_id: String, id: String) -> Result<bool> {
        file::FileMutation.delete_file(ctx, site_id, id).await
    }

    async fn create_file_upload(
        &self,
        ctx: &Context<'_>,
        site_id: String,
        filename: String,
        content_type: String,
    ) -> Result<crate::graphql::types::file::FileUploadUrl> {
        file::FileMutation
            .create_file_upload(ctx, site_id, filename, content_type)
            .await
    }

    async fn restore_file(&self, ctx: &Context<'_>, site_id: String, id: String) -> Result<bool> {
        file::FileMutation.restore_file(ctx, site_id, id).await
    }
}
