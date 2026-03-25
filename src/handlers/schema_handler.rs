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
use crate::models::schema::{Schema, CreateSchema, UpdateSchema};

pub async fn list_schemas(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query_as::<_, Schema>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM schemas WHERE site_id = ? ORDER BY name",
    )
    .bind(&site_id)
    .fetch_all(&pool)
    .await;

    match result {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_schema(
    auth: AuthenticatedUser,
    Path((site_id, schema_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query_as::<_, Schema>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM schemas WHERE site_id = ? AND slug = ?",
    )
    .bind(&site_id)
    .bind(&schema_slug)
    .fetch_optional(&pool)
    .await;

    match result {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Schema not found"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn create_schema(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateSchema>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let definition_str = payload.definition.to_string();
    let id = Uuid::now_v7().to_string();

    let result = sqlx::query(
        "INSERT INTO schemas (id, site_id, name, slug, definition) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&site_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&definition_str)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let item = sqlx::query_as::<_, Schema>(
                "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM schemas WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::CREATED, Json(item)).into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Schema with this name or slug already exists"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn update_schema(
    auth: AuthenticatedUser,
    Path((site_id, schema_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateSchema>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let existing = sqlx::query_as::<_, Schema>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM schemas WHERE site_id = ? AND slug = ?",
    )
    .bind(&site_id)
    .bind(&schema_slug)
    .fetch_optional(&pool)
    .await;

    let existing = match existing {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Schema not found"})),
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
    let definition_str = payload
        .definition
        .map(|s: serde_json::Value| s.to_string())
        .unwrap_or(existing.definition);

    let result = sqlx::query(
        "UPDATE schemas SET name = ?, slug = ?, definition = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&name)
    .bind(&new_slug)
    .bind(&definition_str)
    .bind(&existing.id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let item = sqlx::query_as::<_, Schema>(
                "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM schemas WHERE id = ?",
            )
            .bind(&existing.id)
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

pub async fn delete_schema(
    auth: AuthenticatedUser,
    Path((site_id, schema_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query("DELETE FROM schemas WHERE site_id = ? AND slug = ?")
        .bind(&site_id)
        .bind(&schema_slug)
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
