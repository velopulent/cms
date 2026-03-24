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

use crate::middleware::auth::AuthenticatedUser;
use crate::models::content::{Content, CreateContent, UpdateContent};

#[derive(Deserialize)]
pub struct ListParams {
    pub r#type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

pub async fn list_content(
    _auth: AuthenticatedUser,
    Query(params): Query<ListParams>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let mut query = String::from(
        "SELECT c.id, c.type_id, c.data, c.slug, c.status, c.created_at, c.updated_at, c.published_at
         FROM content c
         JOIN content_types ct ON c.type_id = ct.id
         WHERE 1=1",
    );

    if let Some(type_slug) = &params.r#type {
        query.push_str(&format!(" AND ct.slug = '{}'", type_slug.replace('\'', "''")));
    }
    if let Some(status) = &params.status {
        query.push_str(&format!(" AND c.status = '{}'", status.replace('\'', "''")));
    }
    if let Some(search) = &params.search {
        let escaped = search.replace('\'', "''");
        query.push_str(&format!(
            " AND c.data LIKE '%{}%'",
            escaped
        ));
    }
    query.push_str(" ORDER BY c.updated_at DESC");

    let result = sqlx::query_as::<_, Content>(&query)
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

pub async fn get_content(
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query_as::<_, Content>(
        "SELECT id, type_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await;

    match result {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
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

pub async fn create_content(
    _auth: AuthenticatedUser,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateContent>,
) -> Response {
    let data_str = payload.data.to_string();
    let id = Uuid::now_v7().to_string();

    let result = sqlx::query(
        "INSERT INTO content (id, type_id, data, slug) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.type_id)
    .bind(&data_str)
    .bind(&payload.slug)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let item = sqlx::query_as::<_, Content>(
                "SELECT id, type_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::CREATED, Json(item)).into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Content with this slug already exists for this content type"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn update_content(
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateContent>,
) -> Response {
    let existing = sqlx::query_as::<_, Content>(
        "SELECT id, type_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
    )
    .bind(&id)
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
                "SELECT id, type_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(item)).into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Content with this slug already exists for this content type"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete_content(
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query("DELETE FROM content WHERE id = ?")
        .bind(id)
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

pub async fn publish_content(
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query(
        "UPDATE content SET status = 'published', published_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
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
                "SELECT id, type_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
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

pub async fn unpublish_content(
    _auth: AuthenticatedUser,
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query(
        "UPDATE content SET status = 'draft', updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
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
                "SELECT id, type_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE id = ?",
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
