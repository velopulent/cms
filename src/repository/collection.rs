use sqlx::SqlitePool;

use crate::models::collection::Collection;
use crate::models::content::Content;

pub async fn list(pool: &SqlitePool, site_id: &str) -> Result<Vec<Collection>, sqlx::Error> {
    sqlx::query_as::<_, Collection>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? ORDER BY name",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await
}

pub async fn get_by_slug(
    pool: &SqlitePool,
    site_id: &str,
    slug: &str,
) -> Result<Option<Collection>, sqlx::Error> {
    sqlx::query_as::<_, Collection>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
    )
    .bind(site_id)
    .bind(slug)
    .fetch_optional(pool)
    .await
}

pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Collection>, sqlx::Error> {
    sqlx::query_as::<_, Collection>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    site_id: &str,
    name: &str,
    slug: &str,
    definition: &str,
) -> Result<Collection, sqlx::Error> {
    sqlx::query(
        "INSERT INTO collections (id, site_id, name, slug, definition) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(site_id)
    .bind(name)
    .bind(slug)
    .bind(definition)
    .execute(pool)
    .await?;

    get_by_id(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    slug: &str,
    definition: &str,
) -> Result<Collection, sqlx::Error> {
    sqlx::query(
        "UPDATE collections SET name = ?, slug = ?, definition = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(name)
    .bind(slug)
    .bind(definition)
    .bind(id)
    .execute(pool)
    .await?;

    get_by_id(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete(pool: &SqlitePool, site_id: &str, slug: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM collections WHERE site_id = ? AND slug = ?")
        .bind(site_id)
        .bind(slug)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn get_content_for_migration(
    pool: &SqlitePool,
    collection_id: &str,
) -> Result<Vec<Content>, sqlx::Error> {
    sqlx::query_as::<_, Content>(
        "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE collection_id = ?",
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await
}

pub async fn migrate_content_field_renames(
    pool: &SqlitePool,
    content_items: &[Content],
    rename_map: &std::collections::HashMap<String, String>,
) {
    for content in content_items {
        if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&content.data) {
            if let Some(obj) = data.as_object_mut() {
                let mut renamed = serde_json::Map::new();
                for (key, value) in obj.iter() {
                    let new_key = rename_map
                        .get(key)
                        .cloned()
                        .unwrap_or_else(|| key.clone());
                    renamed.insert(new_key, value.clone());
                }
                let new_data_str = serde_json::to_string(&serde_json::Value::Object(renamed))
                    .unwrap_or_else(|_| content.data.clone());

                let _ = crate::repository::content::update_data(pool, &content.id, &new_data_str)
                    .await;
            }
        }
    }
}
