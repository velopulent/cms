use async_graphql::{Context, Object, Result};

use crate::graphql::context::GqlContext;
use crate::graphql::types::collection::*;
use crate::services::collection::CollectionService;

pub struct CollectionMutation;

#[Object]
impl CollectionMutation {
    pub async fn create_collection(&self, ctx: &Context<'_>, input: CreateCollectionInput) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let definition_str = input.definition.to_string();
        let is_singleton = false;

        let collection = gql_ctx
            .services
            .collection
            .create_collection(site_id, &input.name, &input.slug, &definition_str, is_singleton)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(db_collection_to_gql(collection))
    }

    pub async fn update_collection(
        &self,
        ctx: &Context<'_>,
        slug: String,
        input: UpdateCollectionInput,
    ) -> Result<Collection> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        let definition_str = input.definition.as_ref().map(|s| s.to_string());

        let collection = gql_ctx
            .services
            .collection
            .update_collection(
                site_id,
                &slug,
                input.name.as_deref(),
                input.slug.as_deref(),
                definition_str.as_deref(),
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(db_collection_to_gql(collection))
    }

    pub async fn delete_collection(&self, ctx: &Context<'_>, slug: String) -> Result<bool> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;
        gql_ctx.require_write()?;

        gql_ctx
            .services
            .collection
            .delete_collection(site_id, &slug)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Error: {}", e)))?;

        Ok(true)
    }
}
