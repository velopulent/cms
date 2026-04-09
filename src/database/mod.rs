pub mod backend;
pub mod pool;

use backend::DatabaseBackend;
use pool::DbPool;
use regex::Regex;
use serde_json;
use std::sync::LazyLock;
use tracing::{error, info};

static FILE_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/api/files/([^/]+)(?:/thumbnail)?").expect("Invalid regex"));

pub async fn init_db(database_url: &str) -> Result<DbPool, sqlx::Error> {
    let pool = DbPool::from_url_with_config(
        &crate::config::Config {
            database_url: database_url.to_string(),
            jwt_secret: String::new(),
            bind_address: String::new(),
            grpc_bind_address: String::new(),
            storage_fs_path: None,
            s3_access_key_id: None,
            s3_secret_access_key: None,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            s3_public_url: None,
            max_upload_size_bytes: 0,
            cookie_secure: false,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            hmac_secret: String::new(),
        },
    )
    .await?;

    let backend = pool.backend();
    run_schema(&pool, backend).await?;
    backfill_file_references(&pool).await;
    Ok(pool)
}

pub async fn init_db_with_config(config: &crate::config::Config) -> Result<DbPool, sqlx::Error> {
    let pool = DbPool::from_url_with_config(config).await?;
    let backend = pool.backend();
    run_schema(&pool, backend).await?;
    backfill_file_references(&pool).await;
    Ok(pool)
}

async fn run_schema(pool: &DbPool, backend: DatabaseBackend) -> Result<(), sqlx::Error> {
    let schema = match backend {
        DatabaseBackend::Postgres => include_str!("schema/postgres.sql"),
        DatabaseBackend::MySQL => include_str!("schema/mysql.sql"),
        DatabaseBackend::SQLite => include_str!("schema/sqlite.sql"),
    };

    for statement in schema.split(';').filter(|s| !s.trim().is_empty()) {
        pool.execute(statement).await?;
    }

    Ok(())
}

async fn backfill_file_references(pool: &DbPool) {
    match pool {
        DbPool::Sqlite(sqlite_pool) => backfill_sqlite(sqlite_pool).await,
        DbPool::MySql(mysql_pool) => backfill_mysql(mysql_pool).await,
        DbPool::Postgres(pg_pool) => backfill_postgres(pg_pool).await,
    }
}

async fn backfill_sqlite(pool: &sqlx::SqlitePool) {
    let has_content: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content LIMIT 1)")
        .fetch_one(pool)
        .await
        .unwrap_or(false);

    let has_references: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content_file_references LIMIT 1)")
            .fetch_one(pool)
            .await
            .unwrap_or(false);

    if !has_content || has_references {
        return;
    }

    info!("Backfilling file references...");

    let rows =
        sqlx::query_as::<_, (String, String, String)>("SELECT id, site_id, data FROM content")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    for (content_id, site_id, data_str) in &rows {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(data_str) {
            let file_ids = extract_file_ids_from_value(&data);

            if !file_ids.is_empty() {
                if let Err(e) = batch_insert_references_sqlite(pool, content_id, &file_ids, site_id).await {
                    error!("Failed to backfill references for content {}: {}", content_id, e);
                }
            }
        }
    }

    info!("File reference backfill complete.");
}

async fn batch_insert_references_sqlite(
    pool: &sqlx::SqlitePool,
    content_id: &str,
    file_ids: &[String],
    site_id: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for file_id in file_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO content_file_references (content_id, file_id, site_id) VALUES (?, ?, ?)",
        )
        .bind(content_id)
        .bind(file_id)
        .bind(site_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn backfill_mysql(pool: &sqlx::MySqlPool) {
    let has_content: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content LIMIT 1)")
        .fetch_one(pool)
        .await
        .unwrap_or(false);

    let has_references: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content_file_references LIMIT 1)")
            .fetch_one(pool)
            .await
            .unwrap_or(false);

    if !has_content || has_references {
        return;
    }

    info!("Backfilling file references...");

    let rows =
        sqlx::query_as::<_, (String, String, String)>("SELECT id, site_id, data FROM content")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    for (content_id, site_id, data_str) in &rows {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(data_str) {
            let file_ids = extract_file_ids_from_value(&data);

            if !file_ids.is_empty() {
                if let Err(e) = batch_insert_references_mysql(pool, content_id, &file_ids, site_id).await {
                    error!("Failed to backfill references for content {}: {}", content_id, e);
                }
            }
        }
    }

    info!("File reference backfill complete.");
}

async fn batch_insert_references_mysql(
    pool: &sqlx::MySqlPool,
    content_id: &str,
    file_ids: &[String],
    site_id: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for file_id in file_ids {
        sqlx::query(
            "INSERT IGNORE INTO content_file_references (content_id, file_id, site_id) VALUES (?, ?, ?)",
        )
        .bind(content_id)
        .bind(file_id)
        .bind(site_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn backfill_postgres(pool: &sqlx::PgPool) {
    let has_content: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content LIMIT 1)")
            .fetch_one(pool)
            .await
            .unwrap_or(false);

    let has_references: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content_file_references LIMIT 1)")
            .fetch_one(pool)
            .await
            .unwrap_or(false);

    if !has_content || has_references {
        return;
    }

    info!("Backfilling file references...");

    let rows =
        sqlx::query_as::<_, (String, String, String)>("SELECT id, site_id, data FROM content")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    for (content_id, site_id, data_str) in &rows {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(data_str) {
            let file_ids = extract_file_ids_from_value(&data);

            if !file_ids.is_empty() {
                if let Err(e) = batch_insert_references_postgres(pool, content_id, &file_ids, site_id).await {
                    error!("Failed to backfill references for content {}: {}", content_id, e);
                }
            }
        }
    }

    info!("File reference backfill complete.");
}

async fn batch_insert_references_postgres(
    pool: &sqlx::PgPool,
    content_id: &str,
    file_ids: &[String],
    site_id: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for file_id in file_ids {
        sqlx::query(
            "INSERT INTO content_file_references (content_id, file_id, site_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(content_id)
        .bind(file_id)
        .bind(site_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

fn extract_file_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    let mut ids = Vec::new();
    collect_file_ids(value, &FILE_URL_RE, &mut ids);
    ids.sort();
    ids.dedup();
    ids
}

fn collect_file_ids(value: &serde_json::Value, re: &Regex, ids: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            for cap in re.captures_iter(s) {
                if let Some(m) = cap.get(1) {
                    ids.push(m.as_str().to_string());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_file_ids(item, re, ids);
            }
        }
        serde_json::Value::Object(obj) => {
            for val in obj.values() {
                collect_file_ids(val, re, ids);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_ids_from_string_with_single_file() {
        let value = serde_json::Value::String("/api/files/file123".to_string());
        let ids = extract_file_ids_from_value(&value);
        assert_eq!(ids, vec!["file123"]);
    }

    #[test]
    fn test_extract_file_ids_from_string_with_thumbnail() {
        let value = serde_json::Value::String("/api/files/file123/thumbnail".to_string());
        let ids = extract_file_ids_from_value(&value);
        assert_eq!(ids, vec!["file123"]);
    }

    #[test]
    fn test_extract_file_ids_from_string_with_multiple_files() {
        let value = serde_json::Value::String("/api/files/file1/path /api/files/file2/path".to_string());
        let ids = extract_file_ids_from_value(&value);
        assert_eq!(ids, vec!["file1", "file2"]);
    }

    #[test]
    fn test_extract_file_ids_from_array() {
        let value = serde_json::json!([
            "/api/files/file1",
            "/api/files/file2"
        ]);
        let ids = extract_file_ids_from_value(&value);
        assert_eq!(ids, vec!["file1", "file2"]);
    }

    #[test]
    fn test_extract_file_ids_from_object() {
        let value = serde_json::json!({
            "image": "/api/files/file1",
            "thumbnail": "/api/files/file2/thumbnail"
        });
        let ids = extract_file_ids_from_value(&value);
        assert_eq!(ids, vec!["file1", "file2"]);
    }

    #[test]
    fn test_extract_file_ids_deduplicates() {
        let value = serde_json::Value::String(
            "/api/files/file1/path /api/files/file1/other".to_string()
        );
        let ids = extract_file_ids_from_value(&value);
        assert_eq!(ids, vec!["file1"]);
    }

    #[test]
    fn test_extract_file_ids_nested_structure() {
        let value = serde_json::json!({
            "hero": {
                "image": "/api/files/hero-img",
                "gallery": [
                    "/api/files/img1",
                    "/api/files/img2"
                ]
            }
        });
        let ids = extract_file_ids_from_value(&value);
        assert_eq!(ids, vec!["hero-img", "img1", "img2"]);
    }

    #[test]
    fn test_extract_file_ids_no_matches() {
        let value = serde_json::Value::String("no file URLs here".to_string());
        let ids = extract_file_ids_from_value(&value);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_extract_file_ids_non_string_values() {
        let value = serde_json::json!({
            "count": 42,
            "active": true,
            "data": null
        });
        let ids = extract_file_ids_from_value(&value);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_collect_file_ids_empty_array() {
        let value = serde_json::Value::Array(vec![]);
        let mut ids = Vec::new();
        collect_file_ids(&value, &FILE_URL_RE, &mut ids);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_collect_file_ids_mixed_array() {
        let value = serde_json::json!([
            "/api/files/file1",
            123,
            "/api/files/file2",
            null
        ]);
        let mut ids = Vec::new();
        collect_file_ids(&value, &FILE_URL_RE, &mut ids);
        assert_eq!(ids, vec!["file1", "file2"]);
    }
}