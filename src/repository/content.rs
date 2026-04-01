use sqlx::SqlitePool;

use crate::models::content::Content;

pub struct ListContentParams<'a> {
    pub site_id: &'a str,
    pub collection_slug: Option<&'a str>,
    pub collection_id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub search: Option<&'a str>,
    pub published_only: bool,
}

pub async fn get_by_id(
    pool: &SqlitePool,
    id: &str,
    site_id: &str,
    published_only: bool,
) -> Result<Option<Content>, sqlx::Error> {
    let mut query = String::from(
        "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
         FROM content WHERE id = ? AND site_id = ?",
    );

    if published_only {
        query.push_str(" AND status = 'published'");
    }

    sqlx::query_as::<_, Content>(&query)
        .bind(id)
        .bind(site_id)
        .fetch_optional(pool)
        .await
}

pub async fn get_by_id_any_site(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<Content>, sqlx::Error> {
    sqlx::query_as::<_, Content>(
        "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
         FROM content WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list(
    pool: &SqlitePool,
    params: ListContentParams<'_>,
) -> Result<Vec<Content>, sqlx::Error> {
    let mut query = String::from(
        "SELECT c.id, c.site_id, c.collection_id, c.data, c.slug, c.status, c.created_at, c.updated_at, c.published_at
         FROM content c
         JOIN collections col ON c.collection_id = col.id
         WHERE c.site_id = ?",
    );
    let mut bindings: Vec<String> = vec![params.site_id.to_string()];

    if params.published_only {
        query.push_str(" AND c.status = 'published'");
    }

    if let Some(collection_slug) = params.collection_slug {
        query.push_str(" AND col.slug = ?");
        bindings.push(collection_slug.to_string());
    }

    if let Some(cid) = params.collection_id {
        query.push_str(" AND c.collection_id = ?");
        bindings.push(cid.to_string());
    }

    if let Some(status) = params.status {
        query.push_str(" AND c.status = ?");
        bindings.push(status.to_string());
    }

    if let Some(search) = params.search {
        query.push_str(" AND c.data LIKE ?");
        bindings.push(format!("%{}%", search));
    }

    query.push_str(" ORDER BY c.updated_at DESC");

    let mut q = sqlx::query_as::<_, Content>(&query);
    for b in &bindings {
        q = q.bind(b);
    }

    q.fetch_all(pool).await
}

pub async fn get_by_collection_id(
    pool: &SqlitePool,
    collection_id: &str,
    status: Option<&str>,
    published_only: bool,
) -> Result<Vec<Content>, sqlx::Error> {
    let mut query = String::from(
        "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at
         FROM content WHERE collection_id = ?",
    );
    let mut bindings: Vec<String> = vec![collection_id.to_string()];

    if let Some(s) = status {
        query.push_str(" AND status = ?");
        bindings.push(s.to_string());
    } else if published_only {
        query.push_str(" AND status = 'published'");
    }

    query.push_str(" ORDER BY updated_at DESC");

    let mut q = sqlx::query_as::<_, Content>(&query);
    for b in &bindings {
        q = q.bind(b);
    }

    q.fetch_all(pool).await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    site_id: &str,
    collection_id: &str,
    data: &str,
    slug: &str,
) -> Result<Content, sqlx::Error> {
    sqlx::query(
        "INSERT INTO content (id, site_id, collection_id, data, slug) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(site_id)
    .bind(collection_id)
    .bind(data)
    .bind(slug)
    .execute(pool)
    .await?;

    get_by_id_any_site(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    data: &str,
    slug: &str,
    status: &str,
) -> Result<Content, sqlx::Error> {
    sqlx::query(
        "UPDATE content SET data = ?, slug = ?, status = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(data)
    .bind(slug)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;

    get_by_id_any_site(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update_data(pool: &SqlitePool, id: &str, data: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE content SET data = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(data)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn delete(pool: &SqlitePool, id: &str, site_id: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM content WHERE id = ? AND site_id = ?")
        .bind(id)
        .bind(site_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn publish(pool: &SqlitePool, id: &str, site_id: &str) -> Result<Content, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE content SET status = 'published', published_at = datetime('now'), updated_at = datetime('now') WHERE id = ? AND site_id = ?",
    )
    .bind(id)
    .bind(site_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    get_by_id_any_site(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn unpublish(
    pool: &SqlitePool,
    id: &str,
    site_id: &str,
) -> Result<Content, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE content SET status = 'draft', updated_at = datetime('now') WHERE id = ? AND site_id = ?",
    )
    .bind(id)
    .bind(site_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    get_by_id_any_site(pool, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn sync_file_references(
    pool: &SqlitePool,
    content_id: &str,
    site_id: &str,
    data: &serde_json::Value,
) {
    let file_ids = extract_file_ids(data);

    let _ = sqlx::query("DELETE FROM content_file_references WHERE content_id = ?")
        .bind(content_id)
        .execute(pool)
        .await;

    for file_id in &file_ids {
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO content_file_references (content_id, file_id, site_id) VALUES (?, ?, ?)",
        )
        .bind(content_id)
        .bind(file_id)
        .bind(site_id)
        .execute(pool)
        .await;
    }
}

pub fn extract_file_ids(value: &serde_json::Value) -> Vec<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static FILE_URL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"/api/files/([^/]+)(?:/thumbnail)?").unwrap());

    let mut ids = Vec::new();
    collect_file_ids(value, &FILE_URL_RE, &mut ids);
    ids.sort();
    ids.dedup();
    ids
}

fn collect_file_ids(value: &serde_json::Value, re: &regex::Regex, ids: &mut Vec<String>) {
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
