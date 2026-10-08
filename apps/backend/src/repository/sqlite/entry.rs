use async_trait::async_trait;
use serde_json::Value;
use sqlx::SqlitePool;
use tracing::{debug, error};
use uuid::Uuid;

use crate::models::entry::{Entry, EntryRevision};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{
    EntriesListResult, EntryRepository, ListEntriesParams, RevisionsListResult, UpdateEntryParams,
};

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
        debug!(
            "Fetching entry: id={}, site_id={}, published_only={}",
            id, site_id, published_only
        );

        let mut query = String::from(
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at
              FROM entries WHERE id = ? AND site_id = ?",
        );

        if published_only {
            query.push_str(" AND status = 'published'");
        }

        let result = sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(query.as_str()))
            .bind(id)
            .bind(site_id)
            .fetch_optional(&self.pool)
            .await?;

        debug!(
            "Entry fetch result for id={}, site_id={}: found={}",
            id,
            site_id,
            result.is_some()
        );
        Ok(result)
    }

    async fn get_by_ids(
        &self,
        site_id: &str,
        ids: &[String],
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at FROM entries WHERE site_id = ",
        );
        query.push_bind(site_id).push(" AND id IN (");
        let mut ids_query = query.separated(",");
        for id in ids {
            ids_query.push_bind(id);
        }
        query.push(")");
        if published_only {
            query.push(" AND status = 'published'");
        }
        Ok(query.build_query_as::<Entry>().fetch_all(&self.pool).await?)
    }

    async fn get_by_id_any_site(&self, id: &str) -> Result<Option<Entry>, RepositoryError> {
        let result = sqlx::query_as::<_, Entry>(
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at
             FROM entries WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list(&self, params: ListEntriesParams<'_>) -> Result<EntriesListResult, RepositoryError> {
        debug!(
            "Listing entries: site_id={}, filters: collection_slug={:?}, collection_id={:?}, status={:?}, search={:?}, published_only={}, page={}, per_page={}",
            params.site_id,
            params.collection_slug,
            params.collection_id,
            params.status,
            params.search,
            params.published_only,
            params.page,
            params.per_page
        );

        let mut query = String::from(
            "SELECT e.id, e.site_id, e.collection_id, e.data, e.slug, e.status, e.singleton_collection_id, e.created_at, e.updated_at, CAST(e.version AS TEXT) AS version, e.published_at
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
            let mut q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_query.as_str()));
            for b in &count_bindings {
                q = q.bind(b);
            }
            q.fetch_one(&self.pool).await.map_err(|e| {
                error!("Failed to get entries count: error={}", e);
                e
            })?
        };
        debug!("Total entries count: {}", total);

        debug!(
            "Fetching entries page: page={}, per_page={}, offset={}",
            params.page,
            params.per_page,
            params.page.saturating_sub(1).saturating_mul(params.per_page)
        );
        let offset = params.page.saturating_sub(1).saturating_mul(params.per_page);
        query.push_str(" ORDER BY e.updated_at DESC, e.id DESC LIMIT ? OFFSET ?");

        let mut q = sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(query.as_str()));
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
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at
             FROM entries WHERE collection_id = ?",
        );
        let mut bindings: Vec<String> = vec![collection_id.to_string()];

        if let Some(s) = status {
            query.push_str(" AND status = ?");
            bindings.push(s.to_string());
        }
        if published_only {
            query.push_str(" AND status = 'published'");
        }

        query.push_str(" ORDER BY updated_at DESC");

        let mut q = sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(query.as_str()));
        for b in &bindings {
            q = q.bind(b);
        }

        let result = q.fetch_all(&self.pool).await?;
        Ok(result)
    }

    async fn get_by_collection_ids(
        &self,
        collection_ids: &[String],
        status: Option<&str>,
        published_only: bool,
    ) -> Result<Vec<Entry>, RepositoryError> {
        if collection_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; collection_ids.len()].join(", ");
        let mut query = format!(
            "SELECT ROW_NUMBER() OVER (PARTITION BY collection_id ORDER BY created_at DESC, id DESC) AS collection_rank, id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at
             FROM entries WHERE collection_id IN ({placeholders})"
        );
        let mut bindings: Vec<String> = collection_ids.to_vec();

        if let Some(s) = status {
            query.push_str(" AND status = ?");
            bindings.push(s.to_string());
        }
        if published_only {
            query.push_str(" AND status = 'published'");
        }

        query.push_str(" ORDER BY updated_at DESC");

        // Nested GraphQL reads are bounded per collection. Use the paginated
        // entry query for complete traversal rather than materializing all rows.
        query = format!("SELECT * FROM ({query}) AS bounded WHERE collection_rank <= 200");
        let mut q = sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(query.as_str()));
        for b in &bindings {
            q = q.bind(b);
        }

        Ok(q.fetch_all(&self.pool).await?)
    }

    async fn create(&self, params: crate::repository::traits::CreateEntryParams<'_>) -> Result<Entry, RepositoryError> {
        let crate::repository::traits::CreateEntryParams {
            id,
            site_id,
            collection_id,
            data,
            slug,
            created_by,
            expected_definition,
        } = params;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        lock_definition_tx(&mut tx, collection_id, site_id, expected_definition).await?;
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
        .bind(sqlx::types::Json(&data_json))
        .bind(created_by)
        .execute(&mut *tx)
        .await?;

        sync_references_tx(&mut tx, id, site_id, &data_json).await?;
        let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at FROM entries WHERE id = ?").bind(id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(entry)
    }

    async fn update(&self, params: UpdateEntryParams<'_>) -> Result<Entry, RepositoryError> {
        let UpdateEntryParams {
            expected_definition,
            id,
            site_id,
            data,
            slug,
            status,
            created_by,
            change_summary,
            expected_version,
        } = params;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        let collection_id: Option<String> =
            sqlx::query_scalar("SELECT collection_id FROM entries WHERE id=? AND site_id=?")
                .bind(id)
                .bind(site_id)
                .fetch_optional(&mut *tx)
                .await?;
        let collection_id = collection_id.ok_or(RepositoryError::NotFound)?;
        lock_definition_tx(&mut tx, &collection_id, site_id, expected_definition).await?;
        let result = sqlx::query("UPDATE entries SET data = ?, slug = ?, published_at = CASE WHEN ? = 'draft' THEN NULL WHEN status = 'draft' THEN datetime('now') ELSE published_at END, status = ?, updated_at = datetime('now') WHERE id = ? AND site_id = ? AND (? IS NULL OR CAST(version AS TEXT) = ?)")
            .bind(data).bind(slug).bind(status).bind(status).bind(id).bind(site_id).bind(expected_version)
            .bind(expected_version)
            .execute(&mut *tx).await?;
        if result.rows_affected() == 0 {
            return Err(if expected_version.is_some() {
                RepositoryError::PreconditionFailed
            } else {
                RepositoryError::NotFound
            });
        }

        let next_number: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(revision_number), 0) + 1 FROM entry_revisions WHERE entry_id = ?")
                .bind(id)
                .fetch_one(&mut *tx)
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
        .bind(sqlx::types::Json(&data_json))
        .bind(created_by)
        .bind(change_summary)
        .execute(&mut *tx)
        .await?;

        sync_references_tx(&mut tx, id, site_id, &data_json).await?;
        let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at FROM entries WHERE id = ?").bind(id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(entry)
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
            "UPDATE entries SET status = 'draft', published_at = NULL, updated_at = datetime('now') WHERE id = ? AND site_id = ?",
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
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sync_references_tx(&mut tx, entry_id, site_id, data).await?;
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

        let offset = page.saturating_sub(1).saturating_mul(per_page);
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
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

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

        self.get_by_id_any_site(entry_id)
            .await?
            .ok_or(RepositoryError::NotFound)
    }

    async fn get_singleton_entry(&self, site_id: &str, slug: &str) -> Result<Option<Entry>, RepositoryError> {
        let result = sqlx::query_as::<_, Entry>(
            "SELECT e.id, e.site_id, e.collection_id, e.data, e.slug, e.status, e.singleton_collection_id, e.created_at, e.updated_at, CAST(e.version AS TEXT) AS version, e.published_at
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
        params: crate::repository::traits::UpsertSingletonParams<'_>,
    ) -> Result<Entry, RepositoryError> {
        let crate::repository::traits::UpsertSingletonParams {
            site_id,
            collection_id,
            slug,
            data,
            created_by,
            change_summary,
            expected_definition,
        } = params;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        lock_definition_tx(&mut tx, collection_id, site_id, expected_definition).await?;
        // Serialize first creation and revision allocation on the owning collection.
        sqlx::query("UPDATE collections SET id = id WHERE id = ? AND site_id = ?")
            .bind(collection_id)
            .bind(site_id)
            .execute(&mut *tx)
            .await?;

        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM entries WHERE singleton_collection_id = ? AND site_id = ?")
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

            sqlx::query("UPDATE entries SET data = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(data)
                .bind(&existing_id)
                .execute(&mut *tx)
                .await?;

            let revision_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
                 VALUES (?, ?, ?, ?, ?, datetime('now'), ?)",
            )
            .bind(&revision_id)
            .bind(&existing_id)
            .bind(next_number)
            .bind(sqlx::types::Json(&data_json))
            .bind(created_by)
            .bind(change_summary)
            .execute(&mut *tx)
            .await?;

            sync_references_tx(&mut tx, &existing_id, site_id, &data_json).await?;
            let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at FROM entries WHERE id = ?").bind(&existing_id).fetch_one(&mut *tx).await?;
            tx.commit().await?;
            Ok(entry)
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
                 VALUES (?, ?, 1, ?, ?, datetime('now'), ?)",
            )
            .bind(&revision_id)
            .bind(&id)
            .bind(sqlx::types::Json(&data_json))
            .bind(created_by)
            .bind(change_summary)
            .execute(&mut *tx)
            .await?;

            sync_references_tx(&mut tx, &id, site_id, &data_json).await?;
            let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at FROM entries WHERE id = ?").bind(&id).fetch_one(&mut *tx).await?;
            tx.commit().await?;
            Ok(entry)
        }
    }

    async fn migrate_singleton_field_renames(
        &self,
        site_id: &str,
        collection_id: &str,
        rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        let existing: Option<(String, String)> =
            sqlx::query_as("SELECT id, data FROM entries WHERE singleton_collection_id = ? AND site_id = ?")
                .bind(collection_id)
                .bind(site_id)
                .fetch_optional(&mut *tx)
                .await?;

        if let Some((id, data_str)) = existing
            && let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&data_str)
            && let Some(obj) = data.as_object_mut()
        {
            let mut renamed = serde_json::Map::new();
            for (key, value) in obj.iter() {
                let new_key = rename_map.get(key).cloned().unwrap_or_else(|| key.clone());
                renamed.insert(new_key, value.clone());
            }
            let new_data_str =
                serde_json::to_string(&serde_json::Value::Object(renamed)).unwrap_or_else(|_| data_str.clone());

            sqlx::query("UPDATE entries SET data = ?, updated_at = datetime('now') WHERE id = ?")
                .bind(&new_data_str)
                .bind(&id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

pub use crate::utils::file_references::{extract_file_ids_from_value, extract_file_references_from_value};

async fn lock_definition_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    collection_id: &str,
    site_id: &str,
    expected_definition: Option<&str>,
) -> Result<(), RepositoryError> {
    let found = sqlx::query("UPDATE collections SET id=id WHERE id=? AND site_id=? AND (? IS NULL OR definition=?)")
        .bind(collection_id)
        .bind(site_id)
        .bind(expected_definition)
        .bind(expected_definition)
        .execute(&mut **transaction)
        .await?;
    if found.rows_affected() == 0 {
        return Err(RepositoryError::PreconditionFailed);
    }
    Ok(())
}

pub(crate) async fn sync_references_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    entry_id: &str,
    site_id: &str,
    data: &Value,
) -> Result<(), RepositoryError> {
    let file_references = extract_file_references_from_value(data);
    sqlx::query("DELETE FROM entry_file_references WHERE entry_id = ?")
        .bind(entry_id)
        .execute(&mut **tx)
        .await?;

    for references in file_references.chunks(200) {
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "INSERT INTO entry_file_references (entry_id, file_id, site_id, field_name) ",
        );
        query.push_values(references, |mut row, (file_id, field_name)| {
            row.push_bind(entry_id)
                .push_bind(file_id)
                .push_bind(site_id)
                .push_bind(field_name);
        });
        query.push(" ON CONFLICT DO NOTHING");
        query.build().execute(&mut **tx).await?;
    }

    Ok(())
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    async fn repository() -> SqliteEntryRepository {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations/sqlite").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO users(id,name,email,password_hash) VALUES ('u','u','u@example.com','x')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sites(id,name,created_by) VALUES ('s','s','u')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO collections(id,site_id,name,slug,definition) VALUES ('c','s','c','c','{}')")
            .execute(&pool)
            .await
            .unwrap();
        SqliteEntryRepository::new(pool)
    }

    #[tokio::test]
    async fn schema_preconditions_reject_stale_validations_without_writes() {
        let repo = repository().await;
        let original = repo
            .create(crate::repository::traits::CreateEntryParams {
                id: "e",
                site_id: "s",
                collection_id: "c",
                data: "{}",
                slug: "e",
                created_by: None,
                expected_definition: Some("{}"),
            })
            .await
            .unwrap();
        sqlx::query("UPDATE collections SET definition='{}' WHERE id='c'")
            .execute(&repo.pool)
            .await
            .unwrap();
        let stale = "{\"fields\":[]}";
        assert!(matches!(
            repo.create(crate::repository::traits::CreateEntryParams {
                id: "new",
                site_id: "s",
                collection_id: "c",
                data: "{}",
                slug: "new",
                created_by: None,
                expected_definition: Some(stale)
            })
            .await,
            Err(RepositoryError::PreconditionFailed)
        ));
        assert!(matches!(
            repo.update(UpdateEntryParams {
                id: "e",
                site_id: "s",
                data: "{\"changed\":true}",
                slug: "e",
                status: "draft",
                created_by: None,
                change_summary: None,
                expected_version: Some(&original.version),
                expected_definition: Some(stale),
            })
            .await,
            Err(RepositoryError::PreconditionFailed)
        ));
        assert!(repo.get_by_id("new", "s", false).await.unwrap().is_none());
        let current = repo.get_by_id("e", "s", false).await.unwrap().unwrap();
        assert_eq!(current.data, "{}");
        assert_eq!(current.version, original.version);
        assert_eq!(repo.list_revisions("e", 1, 10).await.unwrap().total, 1);
    }

    #[tokio::test]
    async fn api_upgrade_refuses_legacy_cross_site_content_without_mutating_it() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../migrations/sqlite/20260804000000_initial_schema.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO users(id,name,email,password_hash) VALUES ('u','u','u@example.com','x')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sites(id,name,created_by) VALUES ('s','s','u'),('other','other','u')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO collections(id,site_id,name,slug,definition) VALUES ('c','other','c','c','{}')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entries(id,site_id,collection_id,data,slug) VALUES ('e','s','c','{}','e')")
            .execute(&pool)
            .await
            .unwrap();
        let mut transaction = pool.begin().await.unwrap();
        let error = sqlx::raw_sql(include_str!(
            "../../../migrations/sqlite/20260915000000_api_invariants.sql"
        ))
        .execute(&mut *transaction)
        .await
        .unwrap_err();
        assert!(error.to_string().contains("api_site_ownership_valid"), "{error}");
        transaction.rollback().await.unwrap();
        let entry: (String, String, String) =
            sqlx::query_as("SELECT site_id,collection_id,data FROM entries WHERE id='e'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(entry, ("s".into(), "c".into(), "{}".into()));
    }

    #[tokio::test]
    async fn conditional_updates_reject_stale_versions_without_extra_revisions() {
        let repo = repository().await;
        let original = repo
            .create(crate::repository::traits::CreateEntryParams {
                id: "e",
                site_id: "s",
                collection_id: "c",
                data: "{}",
                slug: "e",
                created_by: None,
                expected_definition: None,
            })
            .await
            .unwrap();
        let update = || UpdateEntryParams {
            expected_definition: None,
            id: "e",
            site_id: "s",
            data: "{}",
            slug: "e",
            status: "draft",
            created_by: None,
            change_summary: None,
            expected_version: Some(&original.version),
        };
        let changed = repo.update(update()).await.unwrap();
        assert_ne!(original.version, changed.version);
        assert!(matches!(
            repo.update(update()).await,
            Err(RepositoryError::PreconditionFailed)
        ));
        assert_eq!(repo.list_revisions("e", 1, 10).await.unwrap().total, 2);
        assert!(
            repo.get_by_collection_id("c", Some("draft"), true)
                .await
                .unwrap()
                .is_empty()
        );
        let published = repo.publish("e", "s").await.unwrap();
        assert_ne!(published.version, changed.version);
    }

    #[tokio::test]
    async fn file_references_preserve_each_field() {
        let repo = repository().await;
        repo.create(crate::repository::traits::CreateEntryParams {
            id: "e",
            site_id: "s",
            collection_id: "c",
            data: "{}",
            slug: "e",
            created_by: None,
            expected_definition: None,
        })
        .await
        .unwrap();
        sqlx::query("INSERT INTO files(id,site_id,filename,original_name,mime_type,size,storage_provider,storage_key) VALUES ('f','s','f','f','image/png',1,'filesystem','f')").execute(&repo.pool).await.unwrap();
        repo.sync_file_references(
            "e",
            "s",
            &serde_json::json!({"hero":"/api/files/f?size=1", "nested":["/api/files/f/thumbnail"]}),
        )
        .await
        .unwrap();
        let fields: Vec<String> =
            sqlx::query_scalar("SELECT field_name FROM entry_file_references ORDER BY field_name")
                .fetch_all(&repo.pool)
                .await
                .unwrap();
        assert_eq!(fields, ["hero", "nested[0]"]);
    }
}
