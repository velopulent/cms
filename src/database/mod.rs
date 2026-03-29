use regex::Regex;
use serde_json;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::str::FromStr;

pub async fn init_db(database_url: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(database_url)
        .expect("Failed to parse database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to connect to database");

    sqlx::query(include_str!("schema.sql"))
        .execute(&pool)
        .await
        .expect("Failed to create tables");

    backfill_media_references(&pool).await;

    pool
}

/// One-time backfill: scan existing content rows and populate content_media_references.
/// Idempotent — skips if references already exist.
async fn backfill_media_references(pool: &SqlitePool) {
    let has_content: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content LIMIT 1)")
            .fetch_one(pool)
            .await
            .unwrap_or(false);

    let has_references: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content_media_references LIMIT 1)")
            .fetch_one(pool)
            .await
            .unwrap_or(false);

    if !has_content || has_references {
        return;
    }

    let media_url_re =
        Regex::new(r"/media/([^/]+)/(?:file|thumbnail)").expect("Invalid regex");

    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, site_id, data FROM content",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (content_id, site_id, data_str) in rows {
        let Ok(data) = serde_json::from_str::<serde_json::Value>(&data_str) else {
            continue;
        };

        let media_ids = extract_media_ids_from_value(&data, &media_url_re);

        for media_id in media_ids {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO content_media_references (content_id, media_id, site_id) VALUES (?, ?, ?)",
            )
            .bind(&content_id)
            .bind(&media_id)
            .bind(&site_id)
            .execute(pool)
            .await;
        }
    }
}

fn extract_media_ids_from_value(value: &serde_json::Value, re: &Regex) -> Vec<String> {
    let mut ids = Vec::new();
    collect_media_ids(value, re, &mut ids);
    ids.sort();
    ids.dedup();
    ids
}

fn collect_media_ids(value: &serde_json::Value, re: &Regex, ids: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(media_id) = s.strip_prefix("media://") {
                ids.push(media_id.to_string());
            }
            for cap in re.captures_iter(s) {
                if let Some(m) = cap.get(1) {
                    ids.push(m.as_str().to_string());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_media_ids(item, re, ids);
            }
        }
        serde_json::Value::Object(obj) => {
            for val in obj.values() {
                collect_media_ids(val, re, ids);
            }
        }
        _ => {}
    }
}
