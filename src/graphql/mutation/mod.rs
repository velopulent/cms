pub mod collection;
pub mod content;
pub mod file;

use async_graphql::{Context, Object, Result};

use crate::graphql::types::collection::*;
use crate::graphql::types::content::*;

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // --- Collections ---

    async fn create_collection(
        &self,
        ctx: &Context<'_>,
        input: CreateCollectionInput,
    ) -> Result<Collection> {
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

    // --- Content ---

    async fn create_content(
        &self,
        ctx: &Context<'_>,
        input: CreateContentInput,
    ) -> Result<Content> {
        content::ContentMutation.create_content(ctx, input).await
    }

    async fn update_content(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateContentInput,
    ) -> Result<Content> {
        content::ContentMutation.update_content(ctx, id, input).await
    }

    async fn delete_content(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        content::ContentMutation.delete_content(ctx, id).await
    }

    async fn publish_content(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        content::ContentMutation.publish_content(ctx, id).await
    }

    async fn unpublish_content(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        content::ContentMutation.unpublish_content(ctx, id).await
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
