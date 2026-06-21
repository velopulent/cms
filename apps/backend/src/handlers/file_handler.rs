use axum::{
    Json,
    body::Body,
    extract::{Extension, Path, Query},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_extra::extract::multipart::Multipart;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::instrument;

#[derive(Deserialize)]
pub struct FileId {
    id: String,
}

use crate::config::Config;
use crate::middleware::auth::{RequestContext, require_site_action};
use crate::models::authorization::Action;
use crate::models::file::{BatchFileIds, FileWithUrl};
use crate::repository::Repository;
use crate::repository::traits::ListFilesParams;
use crate::services::Services;
use crate::services::file::UploadFileRequest;
use crate::storage::{StorageProvider, StorageRegistry};

#[derive(Deserialize, utoipa::IntoParams)]
pub struct FileListParams {
    pub page: Option<i64>,
    pub search: Option<String>,
    pub r#type: Option<String>,
    pub trashed: Option<String>,
}

fn get_storage_for_site(
    site_storage_provider: &str,
    registry: &StorageRegistry,
) -> Result<Arc<dyn StorageProvider>, StatusCode> {
    registry
        .get(site_storage_provider)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[utoipa::path(
    get,
    path = "/api/v1/files",
    params(FileListParams),
    responses(
        (status = 200, description = "List of files"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, params, storage_registry))]
pub async fn list_files(
    ctx: RequestContext,
    Query(params): Query<FileListParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesRead).await {
        return (status, err).into_response();
    }

    let page = params.page.unwrap_or(1).max(1);
    let per_page: i64 = 30;
    let is_trashed = params.trashed.as_deref() == Some("true");

    let list_params = ListFilesParams {
        site_id: &ctx.site_id,
        trashed: is_trashed,
        search: params.search.as_deref(),
        file_type: params.r#type.as_deref(),
        page,
        per_page,
    };

    let storage_provider = services
        .file
        .get_storage_provider(&ctx.site_id)
        .await
        .unwrap_or_else(|_| "filesystem".into());

    match services.file.list_files(list_params).await {
        Ok(result) => {
            let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
                Ok(s) => s,
                Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
            };
            let with_urls: Vec<FileWithUrl> = result
                .items
                .iter()
                .map(|f| services.file.file_to_with_url(f, &*storage))
                .collect();
            (
                StatusCode::OK,
                Json(json!({
                    "items": with_urls,
                    "total": result.total,
                    "page": result.page,
                    "per_page": result.per_page,
                })),
            )
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/files",
    responses(
        (status = 201, description = "File uploaded", body = FileWithUrl),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 413, description = "File too large"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, config, ctx, multipart, storage_registry))]
pub async fn upload_file(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(config): Extension<Config>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
    mut multipart: Multipart,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }
    let site_id = ctx.site_id.clone();

    let storage_provider = services
        .file
        .get_storage_provider(&site_id)
        .await
        .unwrap_or_else(|_| "filesystem".into());

    let mut file_data: Option<Bytes> = None;
    let mut file_name: Option<String> = None;
    let mut file_content_type: Option<String> = None;

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            file_name = field.file_name().map(String::from);
            file_content_type = field.content_type().map(String::from);

            // Stream the field in chunks, enforcing the size cap incrementally so an
            // oversized upload is rejected without first buffering it whole.
            let max = config.max_upload_size_bytes;
            let mut buf: Vec<u8> = Vec::new();
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        if buf.len() + chunk.len() > max {
                            return (
                                StatusCode::PAYLOAD_TOO_LARGE,
                                Json(json!({
                                    "error": format!("File too large. Maximum size is {}MB", max / (1024 * 1024))
                                })),
                            )
                                .into_response();
                        }
                        buf.extend_from_slice(&chunk);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": format!("Failed to read file: {}", e)})),
                        )
                            .into_response();
                    }
                }
            }
            file_data = Some(Bytes::from(buf));
        }
    }

    let file_data = match file_data {
        Some(d) => d,
        None => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file provided"}))).into_response();
        }
    };

    let file_name = sanitize_filename(&file_name.unwrap_or_else(|| "upload".into()));
    let content_type = file_content_type.unwrap_or_else(|| "application/octet-stream".into());
    let created_by = ctx.auth.actor.user_id();

    let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
        Ok(s) => s,
        Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
    };

    match services
        .file
        .upload_file(UploadFileRequest {
            site_id: &site_id,
            data: file_data,
            filename: &file_name,
            content_type: &content_type,
            created_by,
            storage,
            storage_provider: &storage_provider,
        })
        .await
    {
        Ok(file) => (StatusCode::CREATED, Json(file)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Reduce an uploaded filename to a safe basename: strips any path components,
/// control/null bytes, and leading dots (so `../`, `..\`, and dotfiles can't
/// escape or hide). Falls back to `"upload"` if nothing usable remains.
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "upload".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod sanitize_tests {
    use super::sanitize_filename;

    #[test]
    fn strips_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("..\\..\\windows\\system32\\x.dll"), "x.dll");
        assert_eq!(sanitize_filename("/abs/path/photo.jpg"), "photo.jpg");
    }

    #[test]
    fn strips_leading_dots_and_control() {
        assert_eq!(sanitize_filename("...env.png"), "env.png");
        assert_eq!(sanitize_filename("na\0me.txt"), "name.txt");
        assert_eq!(sanitize_filename(".."), "upload");
        assert_eq!(sanitize_filename(""), "upload");
    }

    #[test]
    fn keeps_normal_names() {
        assert_eq!(sanitize_filename("report 2026.pdf"), "report 2026.pdf");
        assert_eq!(sanitize_filename("photo.final.jpg"), "photo.final.jpg");
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/files/{id}",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File item", body = FileWithUrl),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, storage_registry))]
pub async fn get_file(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesRead).await {
        return (status, err).into_response();
    }

    match services.file.get_file(&id, &ctx.site_id).await {
        Ok(Some(file)) => {
            let storage_provider = services
                .file
                .get_storage_provider(&ctx.site_id)
                .await
                .unwrap_or_else(|_| "filesystem".into());
            let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
                Ok(s) => s,
                Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
            };
            let with_url = services.file.file_to_with_url(&file, &*storage);
            (StatusCode::OK, Json(with_url)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/files/{id}",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File soft-deleted"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn delete_file_handler(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    match services.file.soft_delete(&id, &ctx.site_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "File deleted"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/files/{id}/references",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "References found", body = Vec<crate::models::file::FileReference>),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn get_file_references(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesRead).await {
        return (status, err).into_response();
    }

    match services.file.get_file_references(&id, &ctx.site_id).await {
        Ok(refs) => (StatusCode::OK, Json(refs)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/files/{id}/restore",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File restored"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn restore_file(
    ctx: RequestContext,
    Path(FileId { id }): Path<FileId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    match services.file.restore(&id, &ctx.site_id).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "File not found or not deleted"})),
        )
            .into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "File restored"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/files/batch-delete",
    request_body = BatchFileIds,
    responses(
        (status = 200, description = "Files soft-deleted"),
        (status = 400, description = "Bad request"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, body))]
pub async fn batch_delete_files(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    match services.file.batch_soft_delete(&ctx.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"deleted": count}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/files/batch-restore",
    request_body = BatchFileIds,
    responses(
        (status = 200, description = "Files restored"),
        (status = 400, description = "Bad request"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, body))]
pub async fn batch_restore_files(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    match services.file.batch_restore(&ctx.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"restored": count}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/files/batch-permanent-delete",
    request_body = BatchFileIds,
    responses(
        (status = 200, description = "Files permanently deleted"),
        (status = 400, description = "Bad request"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, services, ctx, body, storage_registry))]
pub async fn batch_permanent_delete_files(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::FilesWrite).await {
        return (status, err).into_response();
    }

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    let files = match repository.file.get_deleted_by_ids(&ctx.site_id, &body.ids).await {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to fetch files: {}", e)})),
            )
                .into_response();
        }
    };

    let storage_provider = services
        .file
        .get_storage_provider(&ctx.site_id)
        .await
        .unwrap_or_else(|_| "filesystem".into());
    let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
        Ok(s) => s,
        Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
    };

    for file in &files {
        if let Err(e) = storage.delete(&file.storage_key).await {
            tracing::warn!("Failed to delete file {} from storage: {}", file.id, e);
        }
        if let Some(ref tk) = file.thumbnail_key
            && let Err(e) = storage.delete(tk).await
        {
            tracing::warn!("Failed to delete thumbnail {} from storage: {}", file.id, e);
        }
    }

    match services.file.batch_permanent_delete(&ctx.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"deleted": count}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(services, storage_registry))]
pub async fn serve_file(
    Path(FileId { id }): Path<FileId>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    serve_file_by_key(&id, &services, &storage_registry, false).await
}

#[instrument(skip(services, storage_registry))]
pub async fn serve_file_thumbnail(
    Path(FileId { id }): Path<FileId>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    serve_file_by_key(&id, &services, &storage_registry, true).await
}

async fn serve_file_by_key(
    id: &str,
    services: &Services,
    storage_registry: &StorageRegistry,
    use_thumbnail: bool,
) -> Response {
    let file = match services.file.get_file_any(id).await {
        Ok(Some(f)) => f,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Err(e) => return e.into_response(),
    };

    if file.deleted_at.is_some() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response();
    }

    let storage = match get_storage_for_site(&file.storage_provider, storage_registry) {
        Ok(s) => s,
        Err(status) => return (status, Json(json!({"error": "Storage not configured"}))).into_response(),
    };

    match services.file.serve_file(id, use_thumbnail, storage).await {
        Ok((bytes, content_type, original_name)) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(&content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            headers.insert(header::CONTENT_LENGTH, HeaderValue::from(bytes.len() as u64));
            if use_thumbnail {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
            } else {
                headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
                headers.insert(
                    header::CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!("inline; filename=\"{}\"", original_name))
                        .unwrap_or(HeaderValue::from_static("inline")),
                );
            }
            (StatusCode::OK, headers, Body::from(bytes)).into_response()
        }
        Err(e) => e.into_response(),
    }
}
