use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::models::collection::Collection;
use crate::models::entry::Entry;
use crate::repository::error::RepositoryError;
use crate::repository::traits::CollectionRepository;

pub struct SqliteCollectionRepository {
    pool: SqlitePool,
}

impl SqliteCollectionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CollectionRepository for SqliteCollectionRepository {
    async fn list(&self, site_id: &str) -> Result<Vec<Collection>, RepositoryError> {
        let result = sqlx::query_as::<_, Collection>(
            "SELECT id, site_id, name, slug, definition, is_singleton, created_at, updated_at FROM collections WHERE site_id = ? ORDER BY name",
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list_singletons_only(&self, site_id: &str) -> Result<Vec<Collection>, RepositoryError> {
        let result = sqlx::query_as::<_, Collection>(
            "SELECT id, site_id, name, slug, definition, is_singleton, created_at, updated_at FROM collections WHERE site_id = ? AND is_singleton = 1 ORDER BY name",
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn get_by_slug(&self, site_id: &str, slug: &str) -> Result<Option<Collection>, RepositoryError> {
        let result = sqlx::query_as::<_, Collection>(
            "SELECT id, site_id, name, slug, definition, is_singleton, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
        )
        .bind(site_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Collection>, RepositoryError> {
        let result = sqlx::query_as::<_, Collection>(
            "SELECT id, site_id, name, slug, definition, is_singleton, created_at, updated_at FROM collections WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        name: &str,
        slug: &str,
        definition: &str,
        is_singleton: bool,
    ) -> Result<Collection, RepositoryError> {
        sqlx::query(
            "INSERT INTO collections (id, site_id, name, slug, definition, is_singleton) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(site_id)
        .bind(name)
        .bind(slug)
        .bind(definition)
        .bind(is_singleton)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn update(
        &self,
        id: &str,
        name: &str,
        slug: &str,
        definition: &str,
        rename_map: &std::collections::HashMap<String, String>,
        expected_definition: &str,
    ) -> Result<Collection, RepositoryError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let changed = sqlx::query(
            "UPDATE collections SET name = ?, slug = ?, definition = ?, updated_at = datetime('now') WHERE id = ? AND definition = ?",
        )
        .bind(name)
        .bind(slug)
        .bind(definition)
        .bind(id)
        .bind(expected_definition)
        .execute(&mut *transaction)
        .await?;
        if changed.rows_affected() == 0 {
            return Err(RepositoryError::PreconditionFailed);
        }
        let collection = sqlx::query_as::<_, Collection>(
            "SELECT id,site_id,name,slug,definition,is_singleton,created_at,updated_at FROM collections WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if !rename_map.is_empty() || collection.is_singleton {
            let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM entries WHERE collection_id=? ORDER BY id")
                .bind(id)
                .fetch_all(&mut *transaction)
                .await?;
            for entry_id in ids {
                let current: Option<String> = sqlx::query_scalar("SELECT data FROM entries WHERE id=?")
                    .bind(&entry_id)
                    .fetch_optional(&mut *transaction)
                    .await?;
                let Some(current) = current else {
                    continue;
                };
                let mut data: serde_json::Value =
                    serde_json::from_str(&current).map_err(|error| RepositoryError::Database(error.to_string()))?;
                if !rename_map.is_empty() {
                    let object = data
                        .as_object_mut()
                        .ok_or_else(|| RepositoryError::Database("Stored entry data is not an object".into()))?;
                    let renamed = object
                        .iter()
                        .map(|(key, value)| (rename_map.get(key).unwrap_or(key).clone(), value.clone()))
                        .collect();
                    *object = renamed;
                }
                sqlx::query("UPDATE entries SET data=?,slug=CASE WHEN singleton_collection_id IS NOT NULL THEN ? ELSE slug END,updated_at=datetime('now') WHERE id=?").bind(data.to_string()).bind(slug).bind(&entry_id).execute(&mut *transaction).await?;
                super::entry::sync_references_tx(&mut transaction, &entry_id, &collection.site_id, &data).await?;
            }
        }
        transaction.commit().await?;
        Ok(collection)
    }

    async fn delete(&self, site_id: &str, slug: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM collections WHERE site_id = ? AND slug = ?")
            .bind(site_id)
            .bind(slug)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn get_content_for_migration(&self, collection_id: &str) -> Result<Vec<Entry>, RepositoryError> {
        let result = sqlx::query_as::<_, Entry>(
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, CAST(version AS TEXT) AS version, published_at FROM entries WHERE collection_id = ?",
        )
        .bind(collection_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn migrate_content_field_renames(
        &self,
        entry_items: &[Entry],
        rename_map: &std::collections::HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        // One transaction for the whole migration: per-statement commit overhead
        // dominated this loop, and a partial rename is never left behind.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        for entry in entry_items {
            if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&entry.data)
                && let Some(obj) = data.as_object_mut()
            {
                let mut renamed = serde_json::Map::new();
                for (key, value) in obj.iter() {
                    let new_key = rename_map.get(key).cloned().unwrap_or_else(|| key.clone());
                    renamed.insert(new_key, value.clone());
                }
                let new_data_str =
                    serde_json::to_string(&serde_json::Value::Object(renamed)).unwrap_or_else(|_| entry.data.clone());

                sqlx::query("UPDATE entries SET data = ?, updated_at = datetime('now') WHERE id = ?")
                    .bind(&new_data_str)
                    .bind(&entry.id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}
