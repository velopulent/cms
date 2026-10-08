use async_graphql::{ComplexObject, SimpleObject};

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
    /// Up to 200 most recent entries; page through `site.entries` for more.
    #[graphql(complexity = "200 * child_complexity")]
    async fn entries(
        &self,
        ctx: &async_graphql::Context<'_>,
        status: Option<String>,
    ) -> async_graphql::Result<Vec<Entry>> {
        use crate::graphql::loaders::{EntriesByCollection, EntryLoader};
        use async_graphql::dataloader::DataLoader;

        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        gql_ctx
            .require_site_action_for(
                &self.site_id,
                if status.as_deref() == Some("draft") {
                    crate::models::authorization::Action::ContentPreviewRead
                } else {
                    crate::models::authorization::Action::ContentRead
                },
            )
            .await?;
        let published_only = status.as_deref() != Some("draft");

        // Batched via DataLoader to avoid an N+1 across multiple collections.
        let loader = ctx.data::<DataLoader<EntryLoader>>()?;
        let items = loader
            .load_one(EntriesByCollection {
                site_id: self.site_id.clone(),
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

#[derive(SimpleObject)]
pub struct SingletonGraphql {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: Json,
    pub data: Option<Json>,
    pub entry_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Opaque version of the singleton's content; pass it as `expectedVersion`.
    pub version: Option<String>,
}

pub fn singleton_to_gql(value: crate::models::collection::SingletonResponse) -> SingletonGraphql {
    SingletonGraphql {
        id: value.id,
        site_id: value.site_id,
        name: value.name,
        slug: value.slug,
        definition: Json(value.definition),
        data: value.data.map(Json),
        entry_id: value.entry_id,
        created_at: value.created_at,
        updated_at: value.updated_at,
        version: value.version,
    }
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
