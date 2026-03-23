use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use sqlx::SqlitePool;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::content_type::{ContentType, CreateContentType, UpdateContentType};

pub async fn list_content_types(
    _auth: AuthenticatedUser,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query_as::<_, ContentType>(
        "SELECT id, name, slug, schema_json, created_at, updated_at FROM content_types ORDER BY name",
    )
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
    _auth: AuthenticatedUser,
    Path(slug): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query_as::<_, ContentType>(
        "SELECT id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE slug = ?",
    )
    .bind(&slug)
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
    _auth: AuthenticatedUser,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateContentType>,
) -> Response {
    let schema_str = payload.schema_json.to_string();

    let result = sqlx::query(
        "INSERT INTO content_types (name, slug, schema_json) VALUES (?, ?, ?)",
    )
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&schema_str)
    .execute(&pool)
    .await;

    match result {
        Ok(res) => {
            let id = res.last_insert_rowid();
            let ct = sqlx::query_as::<_, ContentType>(
                "SELECT id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE id = ?",
            )
            .bind(id)
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
    _auth: AuthenticatedUser,
    Path(slug): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateContentType>,
) -> Response {
    let existing = sqlx::query_as::<_, ContentType>(
        "SELECT id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE slug = ?",
    )
    .bind(&slug)
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
    .bind(existing.id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let ct = sqlx::query_as::<_, ContentType>(
                "SELECT id, name, slug, schema_json, created_at, updated_at FROM content_types WHERE id = ?",
            )
            .bind(existing.id)
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
    _auth: AuthenticatedUser,
    Path(slug): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query("DELETE FROM content_types WHERE slug = ?")
        .bind(&slug)
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
