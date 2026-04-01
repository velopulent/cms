use async_graphql::{Context, Object, Result};

use super::context::GqlContext;
use super::types::content::*;

pub struct ContentQuery;

#[Object]
impl ContentQuery {
    async fn content(
        &self,
        ctx: &Context<'_>,
        collection_id: Option<String>,
        status: Option<String>,
        r#type: Option<String>,
        search: Option<String>,
    ) -> Result<Vec<Content>> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let mut query = String::from(
            "SELECT c.id, c.site_id, c.collection_id, c.data, c.slug, c.status, c.created_at, c.updated_at, c.published_at
             FROM content c
             JOIN collections col ON c.collection_id = col.id
             WHERE c.site_id = ?",
        );
        let mut bindings: Vec<String> = vec![site_id.to_string()];

        // API key auth — only published content by default
        query.push_str(" AND c.status = 'published'");

        if let Some(cid) = collection_id {
            query.push_str(" AND c.collection_id = ?");
            bindings.push(cid);
        }

        if let Some(content_type) = &r#type {
            query.push_str(" AND col.slug = ?");
            bindings.push(content_type.clone());
        }

        if let Some(s) = &status {
            if s == "published" || s == "draft" {
                query.push_str(" AND c.status = ?");
                bindings.push(s.clone());
            }
        }

        if let Some(s) = &search {
            query.push_str(" AND c.data LIKE ?");
            bindings.push(format!("%{}%", s));
        }

        query.push_str(" ORDER BY c.updated_at DESC");

        let mut q = sqlx::query_as::<_, crate::models::content::Content>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let items = q
            .fetch_all(&gql_ctx.pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        Ok(items.into_iter().map(db_content_to_gql).collect())
    }

    async fn content_item(&self, ctx: &Context<'_>, id: String) -> Result<Content> {
        let gql_ctx = ctx.data::<GqlContext>()?;
        let site_id = gql_ctx.require_site()?;

        let content = sqlx::query_as::<_, crate::models::content::Content>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
             FROM content WHERE id = ? AND site_id = ? AND status = 'published'",
        )
        .bind(&id)
        .bind(site_id)
        .fetch_optional(&gql_ctx.pool)
        .await
        .map_err(|e| async_graphql::Error::new(format!("Database error: {}", e)))?;

        match content {
            Some(c) => Ok(db_content_to_gql(c)),
            None => Err(async_graphql::Error::new("Content not found")),
        }
    }
}
