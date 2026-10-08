use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::entry::{Entry, EntryRevision};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{
    EntriesListResult, EntryRepository, ListEntriesParams, RevisionsListResult, UpdateEntryParams,
};

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
            "SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at
             FROM entries WHERE id = $1 AND site_id = $2",
        );

        if published_only {
            query.push_str(" AND status = 'published'");
        }

        let result = sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(query.as_str()))
            .bind(id)
            .bind(site_id)
            .fetch_optional(&self.pool)
            .await?;

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
        let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at FROM entries WHERE site_id = ",
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
            "SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at
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
            "SELECT e.id, e.site_id, e.collection_id, e.data::text as data, e.slug, e.status, e.singleton_collection_id, e.created_at::text as created_at, e.updated_at::text as updated_at, e.version::text as version, e.published_at::text as published_at
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
            query.push_str(&format!(" AND e.data::text LIKE ${}", param_index));
            count_query.push_str(&format!(" AND e.data::text LIKE ${}", param_index));
            bindings.push(format!("%{}%", search));
            param_index += 1;
        }

        let total: i64 = {
            let mut q = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_query.as_str()));
            for b in &bindings {
                q = q.bind(b);
            }
            q.fetch_one(&self.pool).await?
        };

        let offset = params.page.saturating_sub(1).saturating_mul(params.per_page);
        query.push_str(&format!(
            " ORDER BY e.updated_at DESC, e.id DESC LIMIT ${} OFFSET ${}",
            param_index,
            param_index + 1
        ));

        let mut q = sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(query.as_str()));
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
            "SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at
             FROM entries WHERE collection_id = $1",
        );
        let mut bindings: Vec<String> = vec![collection_id.to_string()];
        let param_index = 2;

        if let Some(s) = status {
            query.push_str(&format!(" AND status = ${}", param_index));
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
        let placeholders = (1..=collection_ids.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut query = format!(
            "SELECT ROW_NUMBER() OVER (PARTITION BY collection_id ORDER BY created_at DESC, id DESC) AS collection_rank, id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at
             FROM entries WHERE collection_id IN ({placeholders})"
        );
        let mut bindings: Vec<String> = collection_ids.to_vec();

        if let Some(s) = status {
            query.push_str(&format!(" AND status = ${}", collection_ids.len() + 1));
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
        let mut tx = self.pool.begin().await?;

        lock_definition_tx(&mut tx, collection_id, site_id, expected_definition).await?;
        sqlx::query("INSERT INTO entries (id, site_id, collection_id, data, slug) VALUES ($1, $2, $3, $4::jsonb, $5)")
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
             VALUES ($1, $2, 1, $3::jsonb, $4, NOW(), NULL)",
        )
        .bind(&revision_id)
        .bind(id)
        .bind(sqlx::types::Json(&data_json))
        .bind(created_by)
        .execute(&mut *tx)
        .await?;

        sync_references_tx(&mut tx, id, site_id, &data_json).await?;
        let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at FROM entries WHERE id = $1").bind(id).fetch_one(&mut *tx).await?;
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
        let mut tx = self.pool.begin().await?;

        let collection_id: Option<String> =
            sqlx::query_scalar("SELECT collection_id FROM entries WHERE id=$1 AND site_id=$2")
                .bind(id)
                .bind(site_id)
                .fetch_optional(&mut *tx)
                .await?;
        let collection_id = collection_id.ok_or(RepositoryError::NotFound)?;
        lock_definition_tx(&mut tx, &collection_id, site_id, expected_definition).await?;
        let result = sqlx::query("UPDATE entries SET data = $1::jsonb, slug = $2, published_at = CASE WHEN $3 = 'draft' THEN NULL WHEN status = 'draft' THEN NOW() ELSE published_at END, status = $3, updated_at = NOW() WHERE id = $4 AND site_id = $5 AND ($6::text IS NULL OR version::text = $6)")
            .bind(data).bind(slug).bind(status).bind(id).bind(site_id).bind(expected_version)
            .execute(&mut *tx).await?;
        if result.rows_affected() == 0 {
            return Err(if expected_version.is_some() {
                RepositoryError::PreconditionFailed
            } else {
                RepositoryError::NotFound
            });
        }

        let next_number: i64 = sqlx::query_scalar(
            "SELECT (COALESCE(MAX(revision_number), 0) + 1)::BIGINT FROM entry_revisions WHERE entry_id = $1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(Value::Null);
        let revision_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
             VALUES ($1, $2, $3, $4::jsonb, $5, NOW(), $6)",
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
        let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at FROM entries WHERE id = $1").bind(id).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(entry)
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
            sqlx::query("UPDATE entries SET status = 'draft', published_at = NULL, updated_at = NOW() WHERE id = $1 AND site_id = $2")
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
        let mut tx = self.pool.begin().await?;
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
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_revisions WHERE entry_id = $1")
            .bind(entry_id)
            .fetch_one(&self.pool)
            .await?;

        let offset = page.saturating_sub(1).saturating_mul(per_page);
        let items = sqlx::query_as::<_, EntryRevision>(
            "SELECT id, entry_id, revision_number, data, created_by, created_at::text as created_at, change_summary
             FROM entry_revisions WHERE entry_id = $1 ORDER BY revision_number DESC LIMIT $2 OFFSET $3",
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
            "SELECT id, entry_id, revision_number, data, created_by, created_at::text as created_at, change_summary
             FROM entry_revisions WHERE entry_id = $1 AND revision_number = $2",
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
            "SELECT revision_number FROM entry_revisions WHERE entry_id = $1 ORDER BY revision_number DESC LIMIT 1 FOR UPDATE",
        )
        .bind(entry_id)
        .execute(&mut *tx)
        .await?;

        let revision: Option<EntryRevision> = sqlx::query_as(
            "SELECT id, entry_id, revision_number, data, created_by, created_at::text as created_at, change_summary
             FROM entry_revisions WHERE entry_id = $1 AND revision_number = $2",
        )
        .bind(entry_id)
        .bind(revision_number)
        .fetch_optional(&mut *tx)
        .await?;

        let revision = revision.ok_or(RepositoryError::NotFound)?;

        let next_number: i64 = sqlx::query_scalar(
            "SELECT (COALESCE(MAX(revision_number), 0) + 1)::BIGINT FROM entry_revisions WHERE entry_id = $1",
        )
        .bind(entry_id)
        .fetch_one(&mut *tx)
        .await?;

        let data_json = revision.data;
        sqlx::query("UPDATE entries SET data = $1::jsonb, updated_at = NOW() WHERE id = $2")
            .bind(&data_json)
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;

        let new_revision_id = Uuid::now_v7().to_string();
        let change_summary = format!("Restored from revision {}", revision_number);
        sqlx::query(
            "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
             VALUES ($1, $2, $3, $4::jsonb, $5, NOW(), $6)",
        )
        .bind(&new_revision_id)
        .bind(entry_id)
        .bind(next_number)
        .bind(data_json)
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
            "SELECT e.id, e.site_id, e.collection_id, e.data::text as data, e.slug, e.status, e.singleton_collection_id, e.created_at::text as created_at, e.updated_at::text as updated_at, e.version::text as version, e.published_at::text as published_at
             FROM entries e
             JOIN collections c ON c.id = e.singleton_collection_id
             WHERE e.site_id = $1 AND c.slug = $2",
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
        let mut tx = self.pool.begin().await?;

        // Lock the stable parent row, including when no singleton entry exists yet.
        sqlx::query("SELECT id FROM collections WHERE id = $1 AND site_id = $2 FOR UPDATE")
            .bind(collection_id)
            .bind(site_id)
            .execute(&mut *tx)
            .await?;

        lock_definition_tx(&mut tx, collection_id, site_id, expected_definition).await?;
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM entries WHERE singleton_collection_id = $1 AND site_id = $2")
                .bind(collection_id)
                .bind(site_id)
                .fetch_optional(&mut *tx)
                .await?;

        let data_json: serde_json::Value = serde_json::from_str(data).unwrap_or(Value::Null);

        if let Some(existing_id) = existing {
            let next_number: i64 = sqlx::query_scalar(
                "SELECT (COALESCE(MAX(revision_number), 0) + 1)::BIGINT FROM entry_revisions WHERE entry_id = $1",
            )
            .bind(&existing_id)
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query("UPDATE entries SET data = $1::jsonb, updated_at = NOW() WHERE id = $2")
                .bind(data)
                .bind(&existing_id)
                .execute(&mut *tx)
                .await?;

            let revision_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO entry_revisions (id, entry_id, revision_number, data, created_by, created_at, change_summary)
                 VALUES ($1, $2, $3, $4::jsonb, $5, NOW(), $6)",
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
            let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at FROM entries WHERE id = $1").bind(&existing_id).fetch_one(&mut *tx).await?;
            tx.commit().await?;
            Ok(entry)
        } else {
            let id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO entries (id, site_id, collection_id, data, slug, singleton_collection_id) VALUES ($1, $2, $3, $4::jsonb, $5, $6)",
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
                 VALUES ($1, $2, 1, $3::jsonb, $4, NOW(), $5)",
            )
            .bind(&revision_id)
            .bind(&id)
            .bind(sqlx::types::Json(&data_json))
            .bind(created_by)
            .bind(change_summary)
            .execute(&mut *tx)
            .await?;

            sync_references_tx(&mut tx, &id, site_id, &data_json).await?;
            let entry = sqlx::query_as::<_, Entry>("SELECT id, site_id, collection_id, data::text as data, slug, status, singleton_collection_id, created_at::text as created_at, updated_at::text as updated_at, version::text as version, published_at::text as published_at FROM entries WHERE id = $1").bind(&id).fetch_one(&mut *tx).await?;
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
        let mut tx = self.pool.begin().await?;

        let existing: Option<(String, String)> =
            sqlx::query_as("SELECT id, data::text FROM entries WHERE singleton_collection_id = $1 AND site_id = $2")
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

            sqlx::query("UPDATE entries SET data = $1::jsonb, updated_at = NOW() WHERE id = $2")
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    collection_id: &str,
    site_id: &str,
    expected_definition: Option<&str>,
) -> Result<(), RepositoryError> {
    let found: Option<String> = sqlx::query_scalar("SELECT id FROM collections WHERE id=$1 AND site_id=$2 AND ($3::text IS NULL OR definition=$3::jsonb) FOR KEY SHARE").bind(collection_id).bind(site_id).bind(expected_definition).fetch_optional(&mut **transaction).await?;
    if found.is_none() {
        return Err(RepositoryError::PreconditionFailed);
    }
    Ok(())
}

pub(crate) async fn sync_references_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry_id: &str,
    site_id: &str,
    data: &Value,
) -> Result<(), RepositoryError> {
    let file_references = extract_file_references_from_value(data);
    sqlx::query("DELETE FROM entry_file_references WHERE entry_id = $1")
        .bind(entry_id)
        .execute(&mut **tx)
        .await?;

    for references in file_references.chunks(200) {
        let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
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
