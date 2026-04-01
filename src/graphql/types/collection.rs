use async_graphql::{ComplexObject, InputObject, SimpleObject};

use super::content::Content;
use super::json::Json;

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

        let mut query = String::from(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
             FROM content WHERE collection_id = ?",
        );
        let mut bindings: Vec<String> = vec![self.id.clone()];

        if let Some(s) = status {
            query.push_str(" AND status = ?");
            bindings.push(s);
        } else if gql_ctx.site_id.is_some() {
            // API key auth — only show published content by default
            query.push_str(" AND status = 'published'");
        }

        query.push_str(" ORDER BY updated_at DESC");

        let mut q = sqlx::query_as::<_, crate::models::content::Content>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let items = q
            .fetch_all(&gql_ctx.pool)
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
