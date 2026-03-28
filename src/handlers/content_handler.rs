use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::handlers::media_handler::StorageManager;
use crate::middleware::auth::{AuthContext, AuthenticatedUser, check_site_access};
use crate::models::content::{Content, CreateContent, UpdateContent};
use crate::models::media::Media;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListParams {
    pub r#type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/content",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ListParams,
    ),
    responses(
        (status = 200, description = "List of content items", body = Vec<Content>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "content"
)]
pub async fn list_content(
    auth: AuthContext,
    Path(site_id): Path<String>,
    Query(params): Query<ListParams>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    match &auth {
        AuthContext::Jwt { user_id } => {
            if let Err((status, err)) =
                check_site_access(&pool, user_id, &site_id, "viewer").await
            {
                return (status, Json(err)).into_response();
            }
        }
        AuthContext::ApiKey { site_id: key_site_id } => {
            if key_site_id != &site_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "API key does not have access to this site"})),
                )
                    .into_response();
            }
        }
    }

    let mut query = String::from(
        "SELECT c.id, c.site_id, c.collection_id, c.data, c.slug, c.status, c.created_at, c.updated_at, c.published_at
         FROM content c
         JOIN collections s ON c.collection_id = s.id
         WHERE c.site_id = ?",
    );

    let mut bindings: Vec<String> = vec![site_id];

    if matches!(auth, AuthContext::ApiKey { .. }) {
        query.push_str(" AND c.status = 'published'");
    }

    if let Some(collection_slug) = &params.r#type {
        query.push_str(" AND s.slug = ?");
        bindings.push(collection_slug.clone());
    }
    if let Some(status) = &params.status {
        if matches!(auth, AuthContext::Jwt { .. }) {
            query.push_str(" AND c.status = ?");
            bindings.push(status.clone());
        }
    }
    if let Some(search) = &params.search {
        query.push_str(" AND c.data LIKE ?");
        bindings.push(format!("%{}%", search));
    }
    query.push_str(" ORDER BY c.updated_at DESC");

    let mut q = sqlx::query_as::<_, Content>(&query);
    for b in &bindings {
        q = q.bind(b);
    }

    let result = q.fetch_all(&pool).await;

    match result {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/content/{id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Content ID"),
    ),
    responses(
        (status = 200, description = "Content item", body = Content),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Content not found"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "content"
)]
pub async fn get_content(
    auth: AuthContext,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    match &auth {
        AuthContext::Jwt { user_id } => {
            if let Err((status, err)) =
                check_site_access(&pool, user_id, &site_id, "viewer").await
            {
                return (status, Json(err)).into_response();
            }
        }
        AuthContext::ApiKey { site_id: key_site_id } => {
            if key_site_id != &site_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "API key does not have access to this site"})),
                )
                    .into_response();
            }
        }
    }

    let mut query = String::from(
        "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ? AND site_id = ?",
    );

    if matches!(auth, AuthContext::ApiKey { .. }) {
        query.push_str(" AND status = 'published'");
    }

    let result = sqlx::query_as::<_, Content>(&query)
        .bind(&id)
        .bind(&site_id)
        .fetch_optional(&pool)
        .await;

    match result {
        Ok(Some(item)) => {
            let resolved = resolve_content_media(&item, &pool, &storage).await;
            (StatusCode::OK, Json(resolved)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Content not found"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/content",
    params(("site_id" = String, Path, description = "Site ID")),
    request_body = CreateContent,
    responses(
        (status = 201, description = "Content created", body = Content),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 409, description = "Slug already exists"),
    ),
    security(("bearer" = [])),
    tag = "content"
)]
pub async fn create_content(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateContent>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let data_str = payload.data.to_string();
    let id = Uuid::now_v7().to_string();

    let result = sqlx::query(
        "INSERT INTO content (id, site_id, collection_id, data, slug) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&site_id)
    .bind(&payload.collection_id)
    .bind(&data_str)
    .bind(&payload.slug)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let item = sqlx::query_as::<_, Content>(
                "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::CREATED, Json(item)).into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Content with this slug already exists for this collection"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}/content/{id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Content ID"),
    ),
    request_body = UpdateContent,
    responses(
        (status = 200, description = "Content updated", body = Content),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "content"
)]
pub async fn update_content(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateContent>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let existing = sqlx::query_as::<_, Content>(
        "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ? AND site_id = ?",
    )
    .bind(&id)
    .bind(&site_id)
    .fetch_optional(&pool)
    .await;

    let existing = match existing {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Content not found"})),
            )
                .into_response()
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response()
        }
    };

    let data_str = payload
        .data
        .map(|d: serde_json::Value| d.to_string())
        .unwrap_or(existing.data);
    let slug = payload.slug.unwrap_or(existing.slug);
    let status = payload.status.unwrap_or(existing.status);

    let result = sqlx::query(
        "UPDATE content SET data = ?, slug = ?, status = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&data_str)
    .bind(&slug)
    .bind(&status)
    .bind(&id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let item = sqlx::query_as::<_, Content>(
                "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(item)).into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Content with this slug already exists for this collection"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/content/{id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Content ID"),
    ),
    responses(
        (status = 204, description = "Content deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "content"
)]
pub async fn delete_content(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query("DELETE FROM content WHERE id = ? AND site_id = ?")
        .bind(&id)
        .bind(&site_id)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/content/{id}/publish",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Content ID"),
    ),
    responses(
        (status = 200, description = "Content published", body = Content),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Content not found"),
    ),
    security(("bearer" = [])),
    tag = "content"
)]
pub async fn publish_content(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query(
        "UPDATE content SET status = 'published', published_at = datetime('now'), updated_at = datetime('now') WHERE id = ? AND site_id = ?",
    )
    .bind(&id)
    .bind(&site_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Content not found"})),
        )
            .into_response(),
        Ok(_) => {
            let item = sqlx::query_as::<_, Content>(
                "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(item)).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/content/{id}/unpublish",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Content ID"),
    ),
    responses(
        (status = 200, description = "Content unpublished", body = Content),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Content not found"),
    ),
    security(("bearer" = [])),
    tag = "content"
)]
pub async fn unpublish_content(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query(
        "UPDATE content SET status = 'draft', updated_at = datetime('now') WHERE id = ? AND site_id = ?",
    )
    .bind(&id)
    .bind(&site_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Content not found"})),
        )
            .into_response(),
        Ok(_) => {
            let item = sqlx::query_as::<_, Content>(
                "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(item)).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

// --- Media resolution helpers ---

/// Extract all media://<id> references from a JSON value
fn extract_media_ids(value: &serde_json::Value) -> Vec<String> {
    let mut ids = Vec::new();
    extract_media_ids_recursive(value, &mut ids);
    ids
}

fn extract_media_ids_recursive(value: &serde_json::Value, ids: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(media_id) = s.strip_prefix("media://") {
                ids.push(media_id.to_string());
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_media_ids_recursive(item, ids);
            }
        }
        serde_json::Value::Object(obj) => {
            for val in obj.values() {
                extract_media_ids_recursive(val, ids);
            }
        }
        _ => {}
    }
}

/// Resolve media:// URIs in content and return with a _media map
async fn resolve_content_media(
    content: &Content,
    pool: &SqlitePool,
    storage: &StorageManager,
) -> serde_json::Value {
    let data: serde_json::Value = serde_json::from_str(&content.data).unwrap_or_default();
    let media_ids = extract_media_ids(&data);

    let mut media_map = serde_json::Map::new();

    if !media_ids.is_empty() {
        let placeholders = media_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM media WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut q = sqlx::query_as::<_, Media>(&query);
        for id in &media_ids {
            q = q.bind(id);
        }

        if let Ok(media_items) = q.fetch_all(pool).await {
            for m in media_items {
                let url = match m.storage_provider.as_str() {
                    "filesystem" => storage
                        .filesystem
                        .as_ref()
                        .map(|s| s.url(&m.storage_key))
                        .unwrap_or_else(|| format!("/media/{}/file", m.id)),
                    "s3" => storage
                        .s3
                        .as_ref()
                        .map(|s| s.url(&m.storage_key))
                        .unwrap_or_else(|| format!("/media/{}/file", m.id)),
                    _ => format!("/media/{}/file", m.id),
                };

                media_map.insert(
                    m.id.clone(),
                    json!({
                        "id": m.id,
                        "url": url,
                        "thumbnail_url": format!("/media/{}/thumbnail", m.id),
                        "filename": m.filename,
                        "original_name": m.original_name,
                        "mime_type": m.mime_type,
                        "size": m.size,
                        "width": m.width,
                        "height": m.height,
                    }),
                );
            }
        }
    }

    json!({
        "id": content.id,
        "site_id": content.site_id,
        "collection_id": content.collection_id,
        "data": data,
        "slug": content.slug,
        "status": content.status,
        "created_at": content.created_at,
        "updated_at": content.updated_at,
        "published_at": content.published_at,
        "_media": media_map,
    })
}
