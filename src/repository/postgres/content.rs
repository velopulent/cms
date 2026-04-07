use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::LazyLock;

use crate::models::content::Content;
use crate::repository::error::RepositoryError;
use crate::repository::traits::{ContentRepository, ListContentParams};

pub struct PostgresContentRepository {
    pool: PgPool,
}

impl PostgresContentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContentRepository for PostgresContentRepository {
    async fn get_by_id(
        &self,
        id: &str,
        site_id: &str,
        published_only: bool,
    ) -> Result<Option<Content>, RepositoryError> {
        let mut query = String::from(
            "SELECT id, site_id, collection_id, data::text as data, slug, status, created_at::text as created_at, updated_at::text as updated_at, published_at::text as published_at
             FROM content WHERE id = $1 AND site_id = $2",
        );

        if published_only {
            query.push_str(" AND status = 'published'");
        }

        let result = sqlx::query_as::<_, Content>(&query)
            .bind(id)
            .bind(site_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result)
    }

    async fn get_by_id_any_site(&self, id: &str) -> Result<Option<Content>, RepositoryError> {
        let result = sqlx::query_as::<_, Content>(
            "SELECT id, site_id, collection_id, data::text as data, slug, status, created_at::text as created_at, updated_at::text as updated_at, published_at::text as published_at
             FROM content WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list(&self, params: ListContentParams<'_>) -> Result<Vec<Content>, RepositoryError> {
        let mut query = String::from(
            "SELECT c.id, c.site_id, c.collection_id, c.data::text as data, c.slug, c.status, c.created_at::text as created_at, c.updated_at::text as updated_at, c.published_at::text as published_at
             FROM content c
             JOIN collections col ON c.collection_id = col.id
             WHERE c.site_id = $1",
        );
        let mut param_index = 2;
        let mut bindings: Vec<String> = vec![params.site_id.to_string()];

        if params.published_only {
            query.push_str(" AND c.status = 'published'");
        }

        if let Some(collection_slug) = params.collection_slug {
            query.push_str(&format!(" AND col.slug = ${}", param_index));
            bindings.push(collection_slug.to_string());
            param_index += 1;
        }

        if let Some(cid) = params.collection_id {
            query.push_str(&format!(" AND c.collection_id = ${}", param_index));
            bindings.push(cid.to_string());
            param_index += 1;
        }

        if let Some(status) = params.status {
            query.push_str(&format!(" AND c.status = ${}", param_index));
            bindings.push(status.to_string());
            param_index += 1;
        }

        if let Some(search) = params.search {
            query.push_str(&format!(" AND c.data LIKE ${}", param_index));
            bindings.push(format!("%{}%", search));
            param_index += 1;
        }

        query.push_str(" ORDER BY c.updated_at DESC");

        let mut q = sqlx::query_as::<_, Content>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let result = q.fetch_all(&self.pool).await?;
        Ok(result)
    }

    async fn get_by_collection_id(
        &self,
        collection_id: &str,
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Content>, RepositoryError> {
        let mut query = String::from(
            "SELECT id, site_id, collection_id, data::text as data, slug, status, created_at::text as created_at, updated_at::text as updated_at, published_at::text as published_at
             FROM content WHERE collection_id = $1",
        );
        let mut bindings: Vec<String> = vec![collection_id.to_string()];
        let mut param_index = 2;

        if let Some(s) = status {
            query.push_str(&format!(" AND status = ${}", param_index));
            bindings.push(s.to_string());
            param_index += 1;
        } else if published_only {
            query.push_str(" AND status = 'published'");
        }

        query.push_str(" ORDER BY updated_at DESC");

        let mut q = sqlx::query_as::<_, Content>(&query);
        for b in &bindings {
            q = q.bind(b);
        }

        let result = q.fetch_all(&self.pool).await?;
        Ok(result)
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        collection_id: &str,
        data: &str,
        slug: &str,
    ) -> Result<Content, RepositoryError> {
        sqlx::query(
            "INSERT INTO content (id, site_id, collection_id, data, slug) VALUES ($1, $2, $3, $4::jsonb, $5)",
        )
        .bind(id)
        .bind(site_id)
        .bind(collection_id)
        .bind(data)
        .bind(slug)
        .execute(&self.pool)
        .await?;

        self.get_by_id_any_site(id)
            .await?
            .ok_or(RepositoryError::NotFound)
    }

    async fn update(
        &self,
        id: &str,
        data: &str,
        slug: &str,
        status: &str,
    ) -> Result<Content, RepositoryError> {
        sqlx::query(
            "UPDATE content SET data = $1::jsonb, slug = $2, status = $3, updated_at = NOW() WHERE id = $4",
        )
        .bind(data)
        .bind(slug)
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id_any_site(id)
            .await?
            .ok_or(RepositoryError::NotFound)
    }

    async fn update_data(&self, id: &str, data: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query(
            "UPDATE content SET data = $1::jsonb, updated_at = NOW() WHERE id = $2",
        )
        .bind(data)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM content WHERE id = $1 AND site_id = $2")
            .bind(id)
            .bind(site_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn publish(&self, id: &str, site_id: &str) -> Result<Content, RepositoryError> {
        let result = sqlx::query(
            "UPDATE content SET status = 'published', published_at = NOW(), updated_at = NOW() WHERE id = $1 AND site_id = $2",
        )
        .bind(id)
        .bind(site_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        self.get_by_id_any_site(id)
            .await?
            .ok_or(RepositoryError::NotFound)
    }

    async fn unpublish(&self, id: &str, site_id: &str) -> Result<Content, RepositoryError> {
        let result = sqlx::query(
            "UPDATE content SET status = 'draft', updated_at = NOW() WHERE id = $1 AND site_id = $2",
        )
        .bind(id)
        .bind(site_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        self.get_by_id_any_site(id)
            .await?
            .ok_or(RepositoryError::NotFound)
    }

    async fn sync_file_references(
        &self,
        content_id: &str,
        site_id: &str,
        data: &Value,
    ) -> Result<(), RepositoryError> {
        let file_ids = extract_file_ids_from_value(data);

        sqlx::query("DELETE FROM content_file_references WHERE content_id = $1")
            .bind(content_id)
            .execute(&self.pool)
            .await?;

        for file_id in &file_ids {
            sqlx::query(
                "INSERT INTO content_file_references (content_id, file_id, site_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(content_id)
            .bind(file_id)
            .bind(site_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }
}

pub fn extract_file_ids_from_value(value: &Value) -> Vec<String> {
    static FILE_URL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"/api/files/([^/]+)(?:/thumbnail)?").unwrap());

    let mut ids = Vec::new();
    collect_file_ids(value, &FILE_URL_RE, &mut ids);
    ids.sort();
    ids.dedup();
    ids
}

fn collect_file_ids(value: &Value, re: &Regex, ids: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            for cap in re.captures_iter(s) {
                if let Some(m) = cap.get(1) {
                    ids.push(m.as_str().to_string());
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_file_ids(item, re, ids);
            }
        }
        Value::Object(obj) => {
            for val in obj.values() {
                collect_file_ids(val, re, ids);
            }
        }
        _ => {}
    }
}
