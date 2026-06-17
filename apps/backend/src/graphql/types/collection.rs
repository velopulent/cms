use async_graphql::{ComplexObject, InputObject, SimpleObject};

use super::entry::Entry;
use super::json::Json;

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Collection {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: Json,
    pub is_singleton: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[ComplexObject]
impl Collection {
    async fn entry(
        &self,
        ctx: &async_graphql::Context<'_>,
        status: Option<String>,
    ) -> async_graphql::Result<Vec<Entry>> {
        use crate::graphql::loaders::{EntriesByCollection, EntryLoader};
        use async_graphql::dataloader::DataLoader;

        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        let published_only = gql_ctx.site_id.is_some();

        // Batched via DataLoader to avoid an N+1 across multiple collections.
        let loader = ctx.data::<DataLoader<EntryLoader>>()?;
        let items = loader
            .load_one(EntriesByCollection {
                collection_id: self.id.clone(),
                status: status.clone(),
                published_only,
            })
            .await
            .map_err(|e| crate::graphql::internal_error("collection.entry", e))?
            .unwrap_or_default();

        Ok(items.into_iter().map(super::entry::db_entry_to_gql).collect())
    }
}

#[derive(InputObject)]
pub struct CreateCollectionInput {
    pub name: String,
    pub slug: String,
    pub definition: Json,
    pub is_singleton: Option<bool>,
}

#[derive(InputObject)]
pub struct UpdateCollectionInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub definition: Option<Json>,
}

pub fn db_collection_to_gql(c: crate::models::collection::Collection) -> Collection {
    let definition = serde_json::from_str(&c.definition).unwrap_or(serde_json::Value::Object(Default::default()));
    Collection {
        id: c.id,
        site_id: c.site_id,
        name: c.name,
        slug: c.slug,
        definition: Json(definition),
        is_singleton: c.is_singleton,
        created_at: c.created_at,
        updated_at: c.updated_at,
    }
}
