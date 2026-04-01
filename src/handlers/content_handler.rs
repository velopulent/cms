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

use crate::handlers::file_handler::StorageManager;
use crate::middleware::auth::{AuthContext, AuthenticatedUser, check_site_access};
use crate::models::content::{Content, CreateContent, UpdateContent};
use crate::models::file::File;
use crate::repository::content::{self as content_repo, ListContentParams};

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
            if let Err((status, err)) = check_site_access(&pool, user_id, &site_id, "viewer").await
            {
                return (status, Json(err)).into_response();
            }
        }
        AuthContext::ApiKey {
            site_id: key_site_id,
        } => {
            if key_site_id != &site_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "API key does not have access to this site"})),
                )
                    .into_response();
            }
        }
    }

    let published_only = matches!(auth, AuthContext::ApiKey { .. });

    let list_params = ListContentParams {
        site_id: &site_id,
        collection_slug: params.r#type.as_deref(),
        collection_id: None,
        status: if matches!(auth, AuthContext::Jwt { .. }) {
            params.status.as_deref()
        } else {
            None
        },
        search: params.search.as_deref(),
        published_only,
    };

    match content_repo::list(&pool, list_params).await {
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
            if let Err((status, err)) = check_site_access(&pool, user_id, &site_id, "viewer").await
            {
                return (status, Json(err)).into_response();
            }
        }
        AuthContext::ApiKey {
            site_id: key_site_id,
        } => {
            if key_site_id != &site_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "API key does not have access to this site"})),
                )
                    .into_response();
            }
        }
    }

    let published_only = matches!(auth, AuthContext::ApiKey { .. });

    match content_repo::get_by_id(&pool, &id, &site_id, published_only).await {
        Ok(Some(item)) => {
            let resolved = resolve_content_files(&item, &pool, &storage).await;
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

    match content_repo::create(&pool, &id, &site_id, &payload.collection_id, &data_str, &payload.slug).await {
        Ok(item) => {
            content_repo::sync_file_references(&pool, &id, &site_id, &payload.data).await;
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

    let existing = match content_repo::get_by_id(&pool, &id, &site_id, false).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Content not found"})),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let resolved_data = match payload.data {
        Some(d) => d,
        None => serde_json::from_str(&existing.data).unwrap_or(serde_json::Value::Null),
    };
    let data_str = resolved_data.to_string();
    let slug = payload.slug.unwrap_or(existing.slug);
    let status = payload.status.unwrap_or(existing.status);

    match content_repo::update(&pool, &id, &data_str, &slug, &status).await {
        Ok(item) => {
            content_repo::sync_file_references(&pool, &id, &site_id, &resolved_data).await;
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

    match content_repo::delete(&pool, &id, &site_id).await {
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

    match content_repo::publish(&pool, &id, &site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(sqlx::Error::RowNotFound) => (
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

    match content_repo::unpublish(&pool, &id, &site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(sqlx::Error::RowNotFound) => (
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

// --- File resolution helpers (handler-level, uses StorageManager) ---

async fn resolve_content_files(
    content: &Content,
    pool: &SqlitePool,
    storage: &StorageManager,
) -> serde_json::Value {
    let data: serde_json::Value = serde_json::from_str(&content.data).unwrap_or_default();
    let file_ids = content_repo::extract_file_ids(&data);

    let mut file_map = serde_json::Map::new();

    if !file_ids.is_empty() {
        let placeholders = file_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut q = sqlx::query_as::<_, File>(&query);
        for id in &file_ids {
            q = q.bind(id);
        }

        if let Ok(file_items) = q.fetch_all(pool).await {
            for f in file_items {
                let url = match f.storage_provider.as_str() {
                    "s3" => storage
                        .s3
                        .as_ref()
                        .map(|s| s.url(&f.storage_key))
                        .unwrap_or_else(|| format!("/api/files/{}", f.id)),
                    _ => format!("/api/files/{}", f.id),
                };

                file_map.insert(
                    f.id.clone(),
                    json!({
                        "id": f.id,
                        "url": url,
                        "thumbnail_url": f.thumbnail_key.as_ref().map(|_| format!("/api/files/{}/thumbnail", f.id)),
                        "filename": f.filename,
                        "original_name": f.original_name,
                        "mime_type": f.mime_type,
                        "size": f.size,
                        "width": f.width,
                        "height": f.height,
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
        "_files": file_map,
    })
}
