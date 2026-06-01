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

    async fn update(&self, id: &str, name: &str, slug: &str, definition: &str) -> Result<Collection, RepositoryError> {
        sqlx::query(
            "UPDATE collections SET name = ?, slug = ?, definition = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(name)
        .bind(slug)
        .bind(definition)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(RepositoryError::NotFound)
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
            "SELECT id, site_id, collection_id, data, slug, status, singleton_collection_id, created_at, updated_at, published_at FROM entries WHERE collection_id = ?",
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
        for entry in entry_items {
            if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&entry.data)
                && let Some(obj) = data.as_object_mut() {
                    let mut renamed = serde_json::Map::new();
                    for (key, value) in obj.iter() {
                        let new_key = rename_map.get(key).cloned().unwrap_or_else(|| key.clone());
                        renamed.insert(new_key, value.clone());
                    }
                    let new_data_str = serde_json::to_string(&serde_json::Value::Object(renamed))
                        .unwrap_or_else(|_| entry.data.clone());

                    sqlx::query("UPDATE entries SET data = ?, updated_at = datetime('now') WHERE id = ?")
                        .bind(&new_data_str)
                        .bind(&entry.id)
                        .execute(&self.pool)
                        .await?;
                }
        }
        Ok(())
    }
}
