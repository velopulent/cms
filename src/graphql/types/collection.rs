use async_graphql::{ComplexObject, InputObject, SimpleObject};

use super::content::Content;
use super::json::Json;

use crate::repository::content as content_repo;

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Collection {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    pub definition: Json,
    pub created_at: String,
    pub updated_at: String,
}

#[ComplexObject]
impl Collection {
    async fn content(
        &self,
        ctx: &async_graphql::Context<'_>,
        status: Option<String>,
    ) -> async_graphql::Result<Vec<Content>> {
        let gql_ctx = ctx.data::<crate::graphql::context::GqlContext>()?;
        let published_only = gql_ctx.site_id.is_some();

        let items = content_repo::get_by_collection_id(
            &gql_ctx.pool,
            &self.id,
            status.as_deref(),
            published_only,
        )
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(items.into_iter().map(super::content::db_content_to_gql).collect())
    }
}

#[derive(InputObject)]
pub struct CreateCollectionInput {
    pub name: String,
    pub slug: String,
    pub definition: Json,
}

#[derive(InputObject)]
pub struct UpdateCollectionInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub definition: Option<Json>,
}

pub fn db_collection_to_gql(c: crate::models::collection::Collection) -> Collection {
    let definition =
        serde_json::from_str(&c.definition).unwrap_or(serde_json::Value::Object(Default::default()));
    Collection {
        id: c.id,
        site_id: c.site_id,
        name: c.name,
        slug: c.slug,
        definition: Json(definition),
        created_at: c.created_at,
        updated_at: c.updated_at,
    }
}
