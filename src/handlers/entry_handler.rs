use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use crate::handlers::file_handler::StorageManager;
use crate::middleware::auth::{
    HEADER_SITE_ID, Principal, SCOPE_CONTENT_READ, SCOPE_CONTENT_WRITE, require_site_scope,
};
use crate::models::entry::{CreateEntry, Entry, UpdateEntry};
use crate::repository::Repository;
use crate::repository::sqlite::entry::extract_file_ids_from_value;
use crate::repository::traits::ListEntriesParams;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListParams {
    pub r#type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

fn request_site_id(headers: &HeaderMap) -> Option<&str> {
    headers.get(HEADER_SITE_ID).and_then(|value| value.to_str().ok())
}

#[utoipa::path(
    get,
    path = "/api/v1/site/entries",
    params(ListParams),
    responses(
        (status = 200, description = "List of entries", body = Vec<Entry>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, principal, params))]
pub async fn list_entries(
    principal: Principal,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    let published_only = matches!(principal, Principal::SiteToken { .. });
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);

    let list_params = ListEntriesParams {
        site_id: &site.site_id,
        collection_slug: params.r#type.as_deref(),
        collection_id: None,
        status: if matches!(principal, Principal::UserSession { .. }) {
            params.status.as_deref()
        } else {
            None
        },
        search: params.search.as_deref(),
        published_only,
        page,
        per_page,
    };

    match repository.entry.list(list_params).await {
        Ok(result) => {
            let items = resolve_entries_list_files(&result.items, &repository, &site.site_id);
            (
                StatusCode::OK,
                Json(json!({
                    "items": items,
                    "total": result.total,
                    "page": result.page,
                    "per_page": result.per_page,
                })),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/site/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Entry", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, storage, principal))]
pub async fn get_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    let published_only = matches!(principal, Principal::SiteToken { .. });

    match repository.entry.get_by_id(&id, &site.site_id, published_only).await {
        Ok(Some(item)) => {
            let resolved = resolve_entry_files(&item, &repository, &storage).await;
            (StatusCode::OK, Json(resolved)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/entries",
    request_body = CreateEntry,
    responses(
        (status = 201, description = "Entry created", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 409, description = "Slug already exists"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, principal, payload))]
pub async fn create_entry(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<CreateEntry>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    let data_str = payload.data.to_string();
    let id = Uuid::now_v7().to_string();

    match repository
        .entry
        .create(&id, &site.site_id, &payload.collection_id, &data_str, &payload.slug)
        .await
    {
        Ok(item) => {
            let _ = repository
                .entry
                .sync_file_references(&id, &site.site_id, &payload.data)
                .await;
            (StatusCode::CREATED, Json(item)).into_response()
        }
        Err(crate::repository::error::RepositoryError::UniqueViolation(_)) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Entry with this slug already exists for this collection"})),
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
    path = "/api/v1/site/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    request_body = UpdateEntry,
    responses(
        (status = 200, description = "Entry updated", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, principal, payload))]
pub async fn update_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateEntry>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    let existing = match repository.entry.get_by_id(&id, &site.site_id, false).await {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response();
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
        Some(data) => data,
        None => serde_json::from_str(&existing.data).unwrap_or(serde_json::Value::Null),
    };
    let data_str = resolved_data.to_string();
    let slug = payload.slug.unwrap_or(existing.slug);
    let status = payload.status.unwrap_or(existing.status);

    match repository.entry.update(&id, &data_str, &slug, &status).await {
        Ok(item) => {
            let _ = repository
                .entry
                .sync_file_references(&id, &site.site_id, &resolved_data)
                .await;
            (StatusCode::OK, Json(item)).into_response()
        }
        Err(crate::repository::error::RepositoryError::UniqueViolation(_)) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Entry with this slug already exists for this collection"})),
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
    path = "/api/v1/site/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 204, description = "Entry deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, principal))]
pub async fn delete_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.entry.delete(&id, &site.site_id).await {
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
    path = "/api/v1/site/entries/{id}/publish",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Entry published", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, principal))]
pub async fn publish_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.entry.publish(&id, &site.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(crate::repository::error::RepositoryError::NotFound) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response()
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
    path = "/api/v1/site/entries/{id}/unpublish",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Entry unpublished", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, principal))]
pub async fn unpublish_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.entry.unpublish(&id, &site.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(crate::repository::error::RepositoryError::NotFound) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn resolve_entries_files_from_value(
    data: &serde_json::Value,
    repository: &Repository,
    storage: &StorageManager,
    site_id: &str,
) -> serde_json::Value {
    let file_ids = extract_file_ids_from_value(data);

    let mut file_map = serde_json::Map::new();

    if !file_ids.is_empty() {
        if let Ok(file_items) = repository.file.get_by_ids(site_id, &file_ids).await {
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

    let mut result = data.clone();
    if let serde_json::Value::Object(ref mut obj) = result {
        obj.insert("_files".to_string(), serde_json::Value::Object(file_map));
    }
    result
}

async fn resolve_entry_files(entry: &Entry, repository: &Repository, storage: &StorageManager) -> serde_json::Value {
    let data: serde_json::Value = serde_json::from_str(&entry.data).unwrap_or_default();
    let resolved_data = resolve_entries_files_from_value(&data, repository, storage, &entry.site_id).await;
    json!({
        "id": entry.id,
        "site_id": entry.site_id,
        "collection_id": entry.collection_id,
        "data": resolved_data.get("data").cloned().unwrap_or(data),
        "slug": entry.slug,
        "status": entry.status,
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
        "published_at": entry.published_at,
        "_files": resolved_data.get("_files").cloned().unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
    })
}

fn resolve_entries_list_files(items: &[Entry], _repository: &Repository, _site_id: &str) -> Vec<Entry> {
    items.to_vec()
}
