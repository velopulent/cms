use sqlx::SqlitePool;

use crate::models::api_key::ApiKey;

pub async fn list(pool: &SqlitePool, site_id: &str) -> Result<Vec<ApiKey>, sqlx::Error> {
    sqlx::query_as::<_, ApiKey>(
        "SELECT id, site_id, name, key_prefix, permissions, last_used_at, created_at, expires_at
         FROM api_keys WHERE site_id = ? ORDER BY created_at DESC",
    )
    .bind(site_id)
    .fetch_all(pool)
    .await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    site_id: &str,
    name: &str,
    key_hash: &str,
    key_prefix: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO api_keys (id, site_id, name, key_hash, key_prefix) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(site_id)
    .bind(name)
    .bind(key_hash)
    .bind(key_prefix)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str, site_id: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM api_keys WHERE id = ? AND site_id = ?")
        .bind(id)
        .bind(site_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn find_by_prefix(
    pool: &SqlitePool,
    prefix: &str,
) -> Result<Vec<(String, String, String, Option<String>)>, sqlx::Error> {
    sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT id, site_id, key_hash, expires_at FROM api_keys WHERE key_prefix = ?",
    )
    .bind(prefix)
    .fetch_all(pool)
    .await
}

pub async fn update_last_used(pool: &SqlitePool, id: &str) {
    let _ = sqlx::query("UPDATE api_keys SET last_used_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await;
}
