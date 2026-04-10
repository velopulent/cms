use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use sqlx::PgPool;
use std::sync::LazyLock;

use crate::models::entry::Entry;
use crate::repository::error::RepositoryError;
use crate::repository::traits::{EntriesListResult, EntryRepository, ListEntriesParams};

pub struct PostgresEntryRepository {
    pool: PgPool,
}

impl PostgresEntryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EntryRepository for PostgresEntryRepository {
    async fn get_by_id(&self, id: &str, site_id: &str, published_only: bool) -> Result<Option<Entry>, RepositoryError> {
        let mut query = String::from(
            "SELECT id, site_id, collection_id, data::text as data, slug, status, created_at::text as created_at, updated_at::text as updated_at, published_at::text as published_at
             FROM entries WHERE id = $1 AND site_id = $2",
        );

        if published_only {
            query.push_str(" AND status = 'published'");
        }

        let result = sqlx::query_as::<_, Entry>(&query)
            .bind(id)
            .bind(site_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result)
    }

    async fn get_by_id_any_site(&self, id: &str) -> Result<Option<Entry>, RepositoryError> {
        let result = sqlx::query_as::<_, Entry>(
            "SELECT id, site_id, collection_id, data::text as data, slug, status, created_at::text as created_at, updated_at::text as updated_at, published_at::text as published_at
             FROM entries WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list(&self, params: ListEntriesParams<'_>) -> Result<EntriesListResult, RepositoryError> {
        let mut count_query = String::from(
            "SELECT COUNT(*) FROM entries e
             JOIN collections col ON e.collection_id = col.id
             WHERE e.site_id = $1",
        );
        let mut query = String::from(
            "SELECT e.id, e.site_id, e.collection_id, e.data::text as data, e.slug, e.status, e.created_at::text as created_at, e.updated_at::text as updated_at, e.published_at::text as published_at
             FROM entries e
             JOIN collections col ON e.collection_id = col.id
             WHERE e.site_id = $1",
        );
        let mut param_index = 2;
        let mut bindings: Vec<String> = vec![params.site_id.to_string()];

        if params.published_only {
            query.push_str(" AND e.status = 'published'");
            count_query.push_str(" AND e.status = 'published'");
        }

        if let Some(collection_slug) = params.collection_slug {
            query.push_str(&format!(" AND col.slug = ${}", param_index));
            count_query.push_str(&format!(" AND col.slug = ${}", param_index));
            bindings.push(collection_slug.to_string());
            param_index += 1;
        }

        if let Some(cid) = params.collection_id {
            query.push_str(&format!(" AND e.collection_id = ${}", param_index));
            count_query.push_str(&format!(" AND e.collection_id = ${}", param_index));
            bindings.push(cid.to_string());
            param_index += 1;
        }

        if let Some(status) = params.status {
            query.push_str(&format!(" AND e.status = ${}", param_index));
            count_query.push_str(&format!(" AND e.status = ${}", param_index));
            bindings.push(status.to_string());
            param_index += 1;
        }

        if let Some(search) = params.search {
            query.push_str(&format!(" AND e.data LIKE ${}", param_index));
            count_query.push_str(&format!(" AND e.data LIKE ${}", param_index));
            bindings.push(format!("%{}%", search));
            param_index += 1;
        }

        let total: i64 = {
            let mut q = sqlx::query_scalar::<_, i64>(&count_query);
            for b in &bindings {
                q = q.bind(b);
            }
            q.fetch_one(&self.pool).await.unwrap_or(0)
        };

        let offset = (params.page - 1) * params.per_page;
        query.push_str(&format!(
            " ORDER BY e.updated_at DESC LIMIT ${} OFFSET ${}",
            param_index,
            param_index + 1
        ));

        let mut q = sqlx::query_as::<_, Entry>(&query);
        for b in &bindings {
            q = q.bind(b);
        }
        q = q.bind(params.per_page).bind(offset);

        let items = q.fetch_all(&self.pool).await?;

        Ok(EntriesListResult {
            items,
            total,
            page: params.page,
            per_page: params.per_page,
        })
    }

    async fn get_by_collection_id(
        &self,
        collection_id: &str,
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError> {
        let mut query = String::from(
            "SELECT id, site_id, collection_id, data::text as data, slug, status, created_at::text as created_at, updated_at::text as updated_at, published_at::text as published_at
             FROM entries WHERE collection_id = $1",
        );
        let mut bindings: Vec<String> = vec![collection_id.to_string()];
        let param_index = 2;

        if let Some(s) = status {
            query.push_str(&format!(" AND status = ${}", param_index));
            bindings.push(s.to_string());
        } else if published_only {
            query.push_str(" AND status = 'published'");
        }

        query.push_str(" ORDER BY updated_at DESC");

        let mut q = sqlx::query_as::<_, Entry>(&query);
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
    ) -> Result<Entry, RepositoryError> {
        sqlx::query("INSERT INTO entries (id, site_id, collection_id, data, slug) VALUES ($1, $2, $3, $4::jsonb, $5)")
            .bind(id)
            .bind(site_id)
            .bind(collection_id)
            .bind(data)
            .bind(slug)
            .execute(&self.pool)
            .await?;

        self.get_by_id_any_site(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn update(&self, id: &str, data: &str, slug: &str, status: &str) -> Result<Entry, RepositoryError> {
        sqlx::query("UPDATE entries SET data = $1::jsonb, slug = $2, status = $3, updated_at = NOW() WHERE id = $4")
            .bind(data)
            .bind(slug)
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.get_by_id_any_site(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM entries WHERE id = $1 AND site_id = $2")
            .bind(id)
            .bind(site_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn publish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError> {
        let result = sqlx::query(
            "UPDATE entries SET status = 'published', published_at = NOW(), updated_at = NOW() WHERE id = $1 AND site_id = $2",
        )
        .bind(id)
        .bind(site_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        self.get_by_id_any_site(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn unpublish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError> {
        let result =
            sqlx::query("UPDATE entries SET status = 'draft', updated_at = NOW() WHERE id = $1 AND site_id = $2")
                .bind(id)
                .bind(site_id)
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        self.get_by_id_any_site(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn sync_file_references(&self, entry_id: &str, site_id: &str, data: &Value) -> Result<(), RepositoryError> {
        let file_ids = extract_file_ids_from_value(data);

        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM entry_file_references WHERE entry_id = $1")
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;

        for file_id in &file_ids {
            sqlx::query(
                "INSERT INTO entry_file_references (entry_id, file_id, site_id) SELECT $1, id, $2 FROM files WHERE id = $3 AND site_id = $4 ON CONFLICT DO NOTHING",
            )
            .bind(entry_id)
            .bind(site_id)
            .bind(file_id)
            .bind(site_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

pub fn extract_file_ids_from_value(value: &Value) -> Vec<String> {
    static FILE_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/api/files/([^/]+)(?:/thumbnail)?").unwrap());

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
