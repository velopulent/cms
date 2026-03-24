use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::middleware::auth::{AuthenticatedUser, check_site_access};
use crate::models::content_type::{ContentType, CreateContentType, UpdateContentType};

pub async fn list_content_types(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query_as::<_, ContentType>(
        "SELECT id, site_id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE site_id = ? ORDER BY name",
    )
    .bind(&site_id)
    .fetch_all(&pool)
    .await;

    match result {
        Ok(types) => (StatusCode::OK, Json(types)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_content_type(
    auth: AuthenticatedUser,
    Path((site_id, ct_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query_as::<_, ContentType>(
        "SELECT id, site_id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE site_id = ? AND slug = ?",
    )
    .bind(&site_id)
    .bind(&ct_slug)
    .fetch_optional(&pool)
    .await;

    match result {
        Ok(Some(ct)) => (StatusCode::OK, Json(ct)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Content type not found"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn create_content_type(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateContentType>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let schema_str = payload.schema_json.to_string();
    let id = Uuid::now_v7().to_string();

    let result = sqlx::query(
        "INSERT INTO content_types (id, site_id, name, slug, schema_json) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&site_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&schema_str)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let ct = sqlx::query_as::<_, ContentType>(
                "SELECT id, site_id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::CREATED, Json(ct)).into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Content type with this name or slug already exists"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn update_content_type(
    auth: AuthenticatedUser,
    Path((site_id, ct_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateContentType>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let existing = sqlx::query_as::<_, ContentType>(
        "SELECT id, site_id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE site_id = ? AND slug = ?",
    )
    .bind(&site_id)
    .bind(&ct_slug)
    .fetch_optional(&pool)
    .await;

    let existing = match existing {
        Ok(Some(ct)) => ct,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Content type not found"})),
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

    let name = payload.name.unwrap_or(existing.name);
    let new_slug = payload.slug.unwrap_or(existing.slug);
    let schema_str = payload
        .schema_json
        .map(|s: serde_json::Value| s.to_string())
        .unwrap_or(existing.schema_json);

    let result = sqlx::query(
        "UPDATE content_types SET name = ?, slug = ?, schema_json = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&name)
    .bind(&new_slug)
    .bind(&schema_str)
    .bind(&existing.id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let ct = sqlx::query_as::<_, ContentType>(
                "SELECT id, site_id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE id = ?",
            )
            .bind(&existing.id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(ct)).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete_content_type(
    auth: AuthenticatedUser,
    Path((site_id, ct_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query("DELETE FROM content_types WHERE site_id = ? AND slug = ?")
        .bind(&site_id)
        .bind(&ct_slug)
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
