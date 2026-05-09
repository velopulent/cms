use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::LazyLock;
use uuid::Uuid;
use tracing::{error, debug};

use crate::models::entry::{Entry, EntryRevision};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{EntriesListResult, EntryRepository, ListEntriesParams, RevisionsListResult};

static FILE_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/api/files/([^/]+)(?:/thumbnail)?").unwrap());

pub struct SqliteEntryRepository {
    pool: SqlitePool,
}

impl SqliteEntryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EntryRepository for SqliteEntryRepository {
    async fn get_by_id(&self, id: &str, site_id: &str, published_only: bool) -> Result<Option<Entry>, RepositoryError> {
        debug!("Fetching entry: id={}, site_id={}, published_only={}", id, site_id, published_only);
        
        let mut query = String::from(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
              FROM entries WHERE id = ? AND site_id = ?",
        );

        if published_only {
            query.push_str(" AND status = 'published'");
        }

        let result = sqlx::query_as::<_, Entry>(&query)
            .bind(id)
            .bind(site_id)
            .fetch_optional(&self.pool)
            .await?;
        
        debug!("Entry fetch result for id={}, site_id={}: found={}", id, site_id, result.is_some());
        Ok(result)
    }

    async fn get_by_id_any_site(&self, id: &str) -> Result<Option<Entry>, RepositoryError> {
        let result = sqlx::query_as::<_, Entry>(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
             FROM entries WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list(&self, params: ListEntriesParams<'_>) -> Result<EntriesListResult, RepositoryError> {
        debug!("Listing entries: site_id={}, filters: collection_slug={:?}, collection_id={:?}, status={:?}, search={:?}, published_only={}, page={}, per_page={}", 
               params.site_id, params.collection_slug, params.collection_id, params.status, params.search, params.published_only, params.page, params.per_page);
        
        let mut query = String::from(
            "SELECT e.id, e.site_id, e.collection_id, e.data, e.slug, e.status, e.created_at, e.updated_at, e.published_at
              FROM entries e
              JOIN collections col ON e.collection_id = col.id
              WHERE e.site_id = ?",
        );
        let mut count_query = String::from(
            "SELECT COUNT(*) FROM entries e
              JOIN collections col ON e.collection_id = col.id
              WHERE e.site_id = ?",
        );
        let mut bindings: Vec<String> = vec![params.site_id.to_string()];
        let mut count_bindings: Vec<String> = vec![params.site_id.to_string()];

        if params.published_only {
            query.push_str(" AND e.status = 'published'");
            count_query.push_str(" AND e.status = 'published'");
        }

        if let Some(collection_slug) = params.collection_slug {
            query.push_str(" AND col.slug = ?");
            count_query.push_str(" AND col.slug = ?");
            bindings.push(collection_slug.to_string());
            count_bindings.push(collection_slug.to_string());
        }

        if let Some(cid) = params.collection_id {
            query.push_str(" AND e.collection_id = ?");
            count_query.push_str(" AND e.collection_id = ?");
            bindings.push(cid.to_string());
            count_bindings.push(cid.to_string());
        }

        if let Some(status) = params.status {
            query.push_str(" AND e.status = ?");
            count_query.push_str(" AND e.status = ?");
            bindings.push(status.to_string());
            count_bindings.push(status.to_string());
        }

        if let Some(search) = params.search {
            query.push_str(" AND e.data LIKE ?");
            count_query.push_str(" AND e.data LIKE ?");
            bindings.push(format!("%{}%", search));
            count_bindings.push(format!("%{}%", search));
        }

        debug!("Executing count query for entries: site_id={}", params.site_id);
        let total: i64 = {
            let mut q = sqlx::query_scalar::<_, i64>(&count_query);
            for b in &count_bindings {
                q = q.bind(b);
            }
            match q.fetch_one(&self.pool).await {
                Ok(count) => {
                    debug!("Total entries count: {}", count);
                    count
                }
                Err(e) => {
                    error!("Failed to get entries count: error={}", e);
                     return Err(RepositoryError::Database(e.to_string()));
                }
            }
        };

        debug!("Fetching entries page: page={}, per_page={}, offset={}", 
               params.page, params.per_page, (params.page - 1) * params.per_page);
        let offset = (params.page - 1) * params.per_page;
        query.push_str(" ORDER BY e.updated_at DESC LIMIT ? OFFSET ?");

        let mut q = sqlx::query_as::<_, Entry>(&query);
        for b in &bindings {
            q = q.bind(b);
        }
        q = q.bind(params.per_page);
        q = q.bind(offset);

        match q.fetch_all(&self.pool).await {
            Ok(items) => {
                debug!("Retrieved {} entries for site_id={}", items.len(), params.site_id);
                Ok(EntriesListResult {
                    items,
                    total,
                    page: params.page,
                    per_page: params.per_page,
                })
            }
            Err(e) => {
                error!("Failed to fetch entries: error={}", e);
                 Err(RepositoryError::Database(e.to_string()))
            }
        }
    }

    async fn get_by_collection_id(
        &self,
        collection_id: &str,
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError> {
        let mut query = String::from(
            "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
             FROM entries WHERE collection_id = ?",
        );
        let mut bindings: Vec<String> = vec![collection_id.to_string()];

        if let Some(s) = status {
            query.push_str(" AND status = ?");
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
        created_by: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("INSERT INTO entries (id, site_id, collection_id, data, slug) VALUES (?, ?, ?, ?, ?)")
            .bind(id)
            .bind(site_id)
            .bind(collection_id)
            .bind(data)
            .bind(slug)
            .execute(&mut *tx)
            .await?;

        let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(Value::Null);
        let revision_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
             VALUES (?, ?, 1, ?, ?, datetime('now'), NULL)",
        )
        .bind(&revision_id)
        .bind(id)
        .bind(sqlx::types::Json(data_json))
        .bind(created_by)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.get_by_id_any_site(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn update(
        &self,
        id: &str,
        site_id: &str,
        data: &str,
        slug: &str,
        status: &str,
        created_by: Option<&str>,
        change_summary: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let mut tx = self.pool.begin().await?;

        let next_number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(revision_number), 0) + 1 FROM entry_revisions WHERE entry_id = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE entries SET data = ?, slug = ?, status = ?, updated_at = datetime('now') WHERE id = ? AND site_id = ?",
        )
        .bind(data)
        .bind(slug)
        .bind(status)
        .bind(id)
        .bind(site_id)
        .execute(&mut *tx)
        .await?;

        let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(Value::Null);
        let revision_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
             VALUES (?, ?, ?, ?, ?, datetime('now'), ?)",
        )
        .bind(&revision_id)
        .bind(id)
        .bind(next_number)
        .bind(sqlx::types::Json(data_json))
        .bind(created_by)
        .bind(change_summary)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.get_by_id_any_site(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM entries WHERE id = ? AND site_id = ?")
            .bind(id)
            .bind(site_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn publish(&self, id: &str, site_id: &str) -> Result<Entry, RepositoryError> {
        let result = sqlx::query(
            "UPDATE entries SET status = 'published', published_at = datetime('now'), updated_at = datetime('now') WHERE id = ? AND site_id = ?",
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
        let result = sqlx::query(
            "UPDATE entries SET status = 'draft', updated_at = datetime('now') WHERE id = ? AND site_id = ?",
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

    async fn sync_file_references(&self, entry_id: &str, site_id: &str, data: &Value) -> Result<(), RepositoryError> {
        let file_ids = extract_file_ids_from_value(data);

        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM entry_file_references WHERE entry_id = ?")
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;

        for file_id in &file_ids {
            sqlx::query(
                "INSERT OR IGNORE INTO entry_file_references (entry_id, file_id, site_id) SELECT ?, id, ? FROM files WHERE id = ? AND site_id = ?",
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

    async fn list_revisions(
        &self,
        entry_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<RevisionsListResult, RepositoryError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_revisions WHERE entry_id = ?")
            .bind(entry_id)
            .fetch_one(&self.pool)
            .await?;

        let offset = (page - 1) * per_page;
        let items = sqlx::query_as::<_, EntryRevision>(
            "SELECT id, entry_id, revision_number, data, created_by, created_at, change_summary
             FROM entry_revisions WHERE entry_id = ? ORDER BY revision_number DESC LIMIT ? OFFSET ?",
        )
        .bind(entry_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(RevisionsListResult {
            items,
            total,
            page,
            per_page,
        })
    }

    async fn get_revision(
        &self,
        entry_id: &str,
        revision_number: i64,
    ) -> Result<Option<EntryRevision>, RepositoryError> {
        let result = sqlx::query_as::<_, EntryRevision>(
            "SELECT id, entry_id, revision_number, data, created_by, created_at, change_summary
             FROM entry_revisions WHERE entry_id = ? AND revision_number = ?",
        )
        .bind(entry_id)
        .bind(revision_number)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn restore_revision(
        &self,
        entry_id: &str,
        revision_number: i64,
        created_by: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let mut tx = self.pool.begin().await?;

        let revision: Option<EntryRevision> = sqlx::query_as(
            "SELECT id, entry_id, revision_number, data, created_by, created_at, change_summary
             FROM entry_revisions WHERE entry_id = ? AND revision_number = ?",
        )
        .bind(entry_id)
        .bind(revision_number)
        .fetch_optional(&mut *tx)
        .await?;

        let revision = revision.ok_or(RepositoryError::NotFound)?;

        let next_number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(revision_number), 0) + 1 FROM entry_revisions WHERE entry_id = ?",
        )
        .bind(entry_id)
        .fetch_one(&mut *tx)
        .await?;

        let data_str = serde_json::to_string(&revision.data.0).unwrap_or_default();
        sqlx::query("UPDATE entries SET data = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(&data_str)
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;

        let new_revision_id = Uuid::now_v7().to_string();
        let change_summary = format!("Restored from revision {}", revision_number);
        sqlx::query(
            "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
             VALUES (?, ?, ?, ?, ?, datetime('now'), ?)",
        )
        .bind(&new_revision_id)
        .bind(entry_id)
        .bind(next_number)
        .bind(revision.data)
        .bind(created_by)
        .bind(&change_summary)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.get_by_id_any_site(entry_id).await?.ok_or(RepositoryError::NotFound)
    }
}

pub fn extract_file_ids_from_value(value: &Value) -> Vec<String> {
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
