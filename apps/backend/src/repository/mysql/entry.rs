use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use sqlx::MySqlPool;
use std::sync::LazyLock;
use uuid::Uuid;

use crate::models::entry::{Entry, EntryRevision};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{EntriesListResult, EntryRepository, ListEntriesParams, RevisionsListResult};

pub struct MysqlEntryRepository {
    pool: MySqlPool,
}

impl MysqlEntryRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EntryRepository for MysqlEntryRepository {
    async fn get_by_id(&self, id: &str, site_id: &str, published_only: bool) -> Result<Option<Entry>, RepositoryError> {
        let mut query = String::from(
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, published_at
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

        Ok(result)
    }

    async fn get_by_id_any_site(&self, id: &str) -> Result<Option<Entry>, RepositoryError> {
        let result = sqlx::query_as::<_, Entry>(
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, published_at
             FROM entries WHERE id = ?",
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
             WHERE e.site_id = ?",
        );
        let mut query = String::from(
            "SELECT e.id, e.site_id, e.collection_id, e.data, e.slug, e.status, e.singleton_collection_id, e.created_at, e.updated_at, e.published_at
             FROM entries e
             JOIN collections col ON e.collection_id = col.id
             WHERE e.site_id = ?",
        );
        let mut bindings: Vec<String> = vec![params.site_id.to_string()];

        if params.published_only {
            query.push_str(" AND e.status = 'published'");
            count_query.push_str(" AND e.status = 'published'");
        }

        if let Some(collection_slug) = params.collection_slug {
            query.push_str(" AND col.slug = ?");
            count_query.push_str(" AND col.slug = ?");
            bindings.push(collection_slug.to_string());
        }

        if let Some(cid) = params.collection_id {
            query.push_str(" AND e.collection_id = ?");
            count_query.push_str(" AND e.collection_id = ?");
            bindings.push(cid.to_string());
        }

        if let Some(status) = params.status {
            query.push_str(" AND e.status = ?");
            count_query.push_str(" AND e.status = ?");
            bindings.push(status.to_string());
        }

        if let Some(search) = params.search {
            query.push_str(" AND e.data LIKE ?");
            count_query.push_str(" AND e.data LIKE ?");
            bindings.push(format!("%{}%", search));
        }

        let total: i64 = {
            let count_bindings = bindings.clone();
            let mut q = sqlx::query_scalar::<_, i64>(&count_query);
            for b in &count_bindings {
                q = q.bind(b);
            }
            q.fetch_one(&self.pool).await?
        };

        let offset = (params.page - 1) * params.per_page;
        query.push_str(" ORDER BY e.updated_at DESC LIMIT ? OFFSET ?");

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
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, published_at
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
             VALUES (?, ?, 1, ?, ?, NOW(), NULL)",
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

        sqlx::query(
            "SELECT revision_number FROM entry_revisions WHERE entry_id = ? ORDER BY revision_number DESC LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;

        let next_number: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(revision_number), 0) + 1 FROM entry_revisions WHERE entry_id = ?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

        sqlx::query(
            "UPDATE entries SET data = ?, slug = ?, status = ?, updated_at = NOW() WHERE id = ? AND site_id = ?",
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
             VALUES (?, ?, ?, ?, ?, NOW(), ?)",
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
            "UPDATE entries SET status = 'published', published_at = NOW(), updated_at = NOW() WHERE id = ? AND site_id = ?",
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
            sqlx::query("UPDATE entries SET status = 'draft', published_at = NULL, updated_at = NOW() WHERE id = ? AND site_id = ?")
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
                "INSERT IGNORE INTO entry_file_references (entry_id, file_id, site_id) SELECT ?, id, ? FROM files WHERE id = ? AND site_id = ?",
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

        sqlx::query(
            "SELECT revision_number FROM entry_revisions WHERE entry_id = ? ORDER BY revision_number DESC LIMIT 1 FOR UPDATE",
        )
        .bind(entry_id)
        .execute(&mut *tx)
        .await?;

        let revision: Option<EntryRevision> = sqlx::query_as(
            "SELECT id, entry_id, revision_number, data, created_by, created_at, change_summary
             FROM entry_revisions WHERE entry_id = ? AND revision_number = ?",
        )
        .bind(entry_id)
        .bind(revision_number)
        .fetch_optional(&mut *tx)
        .await?;

        let revision = revision.ok_or(RepositoryError::NotFound)?;

        let next_number: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(revision_number), 0) + 1 FROM entry_revisions WHERE entry_id = ?")
                .bind(entry_id)
                .fetch_one(&mut *tx)
                .await?;

        let data_str = serde_json::to_string(&revision.data.0).unwrap_or_default();
        sqlx::query("UPDATE entries SET data = ?, updated_at = NOW() WHERE id = ?")
            .bind(&data_str)
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;

        let new_revision_id = Uuid::now_v7().to_string();
        let change_summary = format!("Restored from revision {}", revision_number);
        sqlx::query(
            "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
             VALUES (?, ?, ?, ?, ?, NOW(), ?)",
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

        self.get_by_id_any_site(entry_id)
            .await?
            .ok_or(RepositoryError::NotFound)
    }

    async fn get_singleton_entry(
        &self,
        site_id: &str,
        slug: &str,
    ) -> Result<Option<Entry>, RepositoryError> {
        let result = sqlx::query_as::<_, Entry>(
            "SELECT e.id, e.site_id, e.collection_id, e.data, e.slug, e.status, e.singleton_collection_id, e.created_at, e.updated_at, e.published_at
             FROM entries e
             JOIN collections c ON c.id = e.singleton_collection_id
             WHERE e.site_id = ? AND c.slug = ?",
        )
        .bind(site_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn upsert_singleton_entry(
        &self,
        site_id: &str,
        collection_id: &str,
        slug: &str,
        data: &str,
        created_by: Option<&str>,
        change_summary: Option<&str>,
    ) -> Result<Entry, RepositoryError> {
        let mut tx = self.pool.begin().await?;

        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM entries WHERE singleton_collection_id = ? AND site_id = ?",
        )
        .bind(collection_id)
        .bind(site_id)
        .fetch_optional(&mut *tx)
        .await?;

        let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(Value::Null);

        if let Some(existing_id) = existing {
            let next_number: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(revision_number), 0) + 1 FROM entry_revisions WHERE entry_id = ?",
            )
            .bind(&existing_id)
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query("UPDATE entries SET data = ?, updated_at = NOW() WHERE id = ?")
                .bind(data)
                .bind(&existing_id)
                .execute(&mut *tx)
                .await?;

            let revision_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
                 VALUES (?, ?, ?, ?, ?, NOW(), ?)",
            )
            .bind(&revision_id)
            .bind(&existing_id)
            .bind(next_number)
            .bind(sqlx::types::Json(data_json))
            .bind(created_by)
            .bind(change_summary)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
            self.get_by_id_any_site(&existing_id)
                .await?
                .ok_or(RepositoryError::NotFound)
        } else {
            let id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO entries (id, site_id, collection_id, data, slug, singleton_collection_id) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(site_id)
            .bind(collection_id)
            .bind(data)
            .bind(slug)
            .bind(collection_id)
            .execute(&mut *tx)
            .await?;

            let revision_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
                 VALUES (?, ?, 1, ?, ?, NOW(), ?)",
            )
            .bind(&revision_id)
            .bind(&id)
            .bind(sqlx::types::Json(data_json))
            .bind(created_by)
            .bind(change_summary)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
            self.get_by_id_any_site(&id).await?.ok_or(RepositoryError::NotFound)
        }
    }

    async fn migrate_singleton_field_renames(
        &self,
        site_id: &str,
        collection_id: &str,
        rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;

        let existing: Option<(String, String)> = sqlx::query_as(
            "SELECT id, data FROM entries WHERE singleton_collection_id = ? AND site_id = ?",
        )
        .bind(collection_id)
        .bind(site_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some((id, data_str)) = existing
            && let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&data_str)
                && let Some(obj) = data.as_object_mut() {
                    let mut renamed = serde_json::Map::new();
                    for (key, value) in obj.iter() {
                        let new_key = rename_map.get(key).cloned().unwrap_or_else(|| key.clone());
                        renamed.insert(new_key, value.clone());
                    }
                    let new_data_str = serde_json::to_string(&serde_json::Value::Object(renamed))
                        .unwrap_or_else(|_| data_str.clone());

                    sqlx::query("UPDATE entries SET data = ?, updated_at = NOW() WHERE id = ?")
                        .bind(&new_data_str)
                        .bind(&id)
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
