use axum::{
    Json,
    body::Body,
    extract::{Extension, Path, Query},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_extra::extract::multipart::Multipart;
use bytes::Bytes;
use image::{DynamicImage, ImageEncoder, ImageReader};
use serde::Deserialize;
use serde_json::json;
use std::io::Cursor;
use tracing::instrument;
use uuid::Uuid;

use crate::config::Config;
use crate::middleware::auth::{
    HEADER_SITE_ID, Principal, SCOPE_ASSETS_READ, SCOPE_ASSETS_WRITE, extract_user_id, require_site_scope,
};
use crate::models::file::{BatchFileIds, File, FileWithUrl};
use crate::repository::Repository;
use crate::repository::traits::ListFilesParams;
use crate::storage::{FileSystemStorage, S3Storage};

#[derive(Clone)]
pub struct StorageManager {
    pub filesystem: Option<FileSystemStorage>,
    pub s3: Option<S3Storage>,
}

impl StorageManager {
    pub fn has_s3(&self) -> bool {
        self.s3.is_some()
    }

    pub fn has_any(&self) -> bool {
        self.filesystem.is_some() || self.s3.is_some()
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct FileListParams {
    pub page: Option<i64>,
    pub search: Option<String>,
    pub r#type: Option<String>,
    pub trashed: Option<String>,
}

fn file_to_with_url(file: &File, storage: &StorageManager) -> FileWithUrl {
    let url = match file.storage_provider.as_str() {
        "s3" => storage
            .s3
            .as_ref()
            .map(|s| s.url(&file.storage_key))
            .unwrap_or_else(|| format!("/api/files/{}", file.id)),
        _ => format!("/api/files/{}", file.id),
    };

    let thumbnail_url = file
        .thumbnail_key
        .as_ref()
        .map(|_| format!("/api/files/{}/thumbnail", file.id));

    FileWithUrl {
        id: file.id.clone(),
        site_id: file.site_id.clone(),
        filename: file.filename.clone(),
        original_name: file.original_name.clone(),
        mime_type: file.mime_type.clone(),
        size: file.size,
        storage_provider: file.storage_provider.clone(),
        storage_key: file.storage_key.clone(),
        thumbnail_key: file.thumbnail_key.clone(),
        width: file.width,
        height: file.height,
        deleted_at: file.deleted_at.clone(),
        created_by: file.created_by.clone(),
        created_at: file.created_at.clone(),
        url,
        thumbnail_url,
    }
}

fn request_site_id(headers: &HeaderMap) -> Option<&str> {
    headers.get(HEADER_SITE_ID).and_then(|value| value.to_str().ok())
}

fn mime_to_ext(mime: &str) -> &str {
    match mime {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

fn generate_thumbnail(img: &DynamicImage) -> Option<(Vec<u8>, String)> {
    let thumb = img.resize(260, 260, image::imageops::FilterType::Lanczos3);
    let rgba = thumb.to_rgba8();
    let mut bytes = Vec::new();
    let encoder = image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut bytes, 7, 60);
    encoder
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some((bytes, "image/avif".into()))
}

async fn store_file(
    storage: &StorageManager,
    provider: &str,
    key: &str,
    data: Bytes,
    content_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match provider {
        "filesystem" => {
            if let Some(fs) = &storage.filesystem {
                fs.put(key, data, content_type).await
            } else {
                Err("Filesystem storage not available".into())
            }
        }
        "s3" => {
            if let Some(s3) = &storage.s3 {
                s3.put(key, data, content_type).await
            } else {
                Err("S3 storage not available".into())
            }
        }
        _ => Err(format!("Unknown storage provider: {}", provider).into()),
    }
}

async fn read_from_storage(
    storage: &StorageManager,
    provider: &str,
    key: &str,
) -> Result<Bytes, Box<dyn std::error::Error>> {
    match provider {
        "filesystem" => {
            if let Some(fs) = &storage.filesystem {
                fs.get(key).await
            } else {
                Err("Filesystem storage not available".into())
            }
        }
        "s3" => {
            if let Some(s3) = &storage.s3 {
                s3.get(key).await
            } else {
                Err("S3 storage not available".into())
            }
        }
        _ => Err(format!("Unknown storage provider: {}", provider).into()),
    }
}

async fn remove_from_storage(
    storage: &StorageManager,
    provider: &str,
    key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match provider {
        "filesystem" => {
            if let Some(fs) = &storage.filesystem {
                fs.delete(key).await
            } else {
                Err("Filesystem storage not available".into())
            }
        }
        "s3" => {
            if let Some(s3) = &storage.s3 {
                s3.delete(key).await
            } else {
                Err("S3 storage not available".into())
            }
        }
        _ => Err(format!("Unknown storage provider: {}", provider).into()),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/site/files",
    params(FileListParams),
    responses(
        (status = 200, description = "List of files"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, storage, principal, params))]
pub async fn list_files(
    principal: Principal,
    headers: HeaderMap,
    Query(params): Query<FileListParams>,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    let page = params.page.unwrap_or(1).max(1);
    let per_page: i64 = 30;
    let is_trashed = params.trashed.as_deref() == Some("true");

    let list_params = ListFilesParams {
        site_id: &site.site_id,
        trashed: is_trashed,
        search: params.search.as_deref(),
        file_type: params.r#type.as_deref(),
        page,
        per_page,
    };

    match repository.file.list(list_params).await {
        Ok(result) => {
            let with_urls: Vec<FileWithUrl> = result
                .items
                .into_iter()
                .map(|f| file_to_with_url(&f, &storage))
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
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/files",
    responses(
        (status = 201, description = "File uploaded", body = FileWithUrl),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 413, description = "File too large"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, storage, config, principal, multipart))]
pub async fn upload_file(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
    Extension(config): Extension<Config>,
    mut multipart: Multipart,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };
    let site_id = site.site_id;

    let mut storage_provider = repository
        .file
        .get_storage_provider(&site_id)
        .await
        .unwrap_or_else(|_| "filesystem".into());

    let mut file_data: Option<Bytes> = None;
    let mut file_name: Option<String> = None;
    let mut file_content_type: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "storage_provider" {
            if let Ok(val) = field.text().await {
                if val == "s3" && storage.has_s3() {
                    storage_provider = val;
                }
            }
        } else if name == "file" {
            file_name = field.file_name().map(String::from);
            file_content_type = field.content_type().map(String::from);

            match field.bytes().await {
                Ok(bytes) => {
                    if bytes.len() > config.max_upload_size_bytes {
                        return (
                            StatusCode::PAYLOAD_TOO_LARGE,
                            Json(json!({
                                "error": format!(
                                    "File too large. Maximum size is {}MB",
                                    config.max_upload_size_bytes / (1024 * 1024)
                                )
                            })),
                        )
                            .into_response();
                    }
                    file_data = Some(bytes);
                }
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Failed to read file: {}", e)})),
                    )
                        .into_response();
                }
            }
        }
    }

    let file_data = match file_data {
        Some(d) => d,
        None => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file provided"}))).into_response();
        }
    };

    let original_name = file_name.unwrap_or_else(|| "upload".into());
    let content_type = file_content_type.unwrap_or_else(|| "application/octet-stream".into());
    let file_size = file_data.len() as i64;

    let file_id = Uuid::now_v7().to_string();
    let ext = std::path::Path::new(&original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let filename = if ext.is_empty() {
        format!("{}.{}", &file_id[..8], mime_to_ext(&content_type))
    } else {
        format!("{}.{}", &file_id[..8], ext)
    };

    let storage_key = format!("s_{}/f_{}/{}", site_id, file_id, filename);
    let mime_type = content_type.clone();

    let mut width: Option<i32> = None;
    let mut height: Option<i32> = None;
    let mut thumbnail_data: Option<(Vec<u8>, String)> = None;
    let mut thumbnail_key: Option<String> = None;

    if mime_type.starts_with("image/") {
        let data_clone = file_data.clone();
        let file_id_owned = file_id.clone();
        let site_id_owned = site_id.clone();

        let result = tokio::task::spawn_blocking(move || {
            let mut w: Option<i32> = None;
            let mut h: Option<i32> = None;
            let mut tdata: Option<(Vec<u8>, String)> = None;
            let mut tkey: Option<String> = None;

            if let Ok(reader) = ImageReader::new(Cursor::new(&data_clone)).with_guessed_format() {
                if let Ok(img) = reader.decode() {
                    w = Some(img.width() as i32);
                    h = Some(img.height() as i32);

                    if let Some((thumb_bytes, thumb_mime)) = generate_thumbnail(&img) {
                        tkey = Some(format!(
                            "s_{}/f_{}/thumb_{}.avif",
                            site_id_owned,
                            file_id_owned,
                            &file_id_owned[..8]
                        ));
                        tdata = Some((thumb_bytes, thumb_mime));
                    }
                }
            }

            (w, h, tdata, tkey)
        })
        .await;

        if let Ok((w, h, tdata, tkey)) = result {
            width = w;
            height = h;
            thumbnail_data = tdata;
            thumbnail_key = tkey;
        }
    }

    if let Err(e) = store_file(&storage, &storage_provider, &storage_key, file_data, &mime_type).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store file: {}", e)})),
        )
            .into_response();
    }

    if let (Some((thumb_data, thumb_mime)), Some(thumb_key)) = (&thumbnail_data, &thumbnail_key) {
        let _ = store_file(
            &storage,
            &storage_provider,
            thumb_key,
            Bytes::from(thumb_data.clone()),
            thumb_mime,
        )
        .await;
    }

    let thumb_key_str = thumbnail_key.as_deref();

    let created_by = extract_user_id(&principal);

    match repository
        .file
        .create(
            &file_id,
            &site_id,
            &filename,
            &original_name,
            &mime_type,
            file_size,
            &storage_provider,
            &storage_key,
            thumb_key_str,
            width,
            height,
            created_by,
        )
        .await
    {
        Ok(file) => {
            let with_url = file_to_with_url(&file, &storage);
            (StatusCode::CREATED, Json(with_url)).into_response()
        }
        Err(err) => {
            let _ = remove_from_storage(&storage, &storage_provider, &storage_key).await;
            if let Some(ref tk) = thumbnail_key {
                let _ = remove_from_storage(&storage, &storage_provider, tk).await;
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/site/files/{id}",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File item", body = FileWithUrl),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, storage, principal))]
pub async fn get_file(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.file.get_by_id(&id, &site.site_id).await {
        Ok(Some(file)) => {
            let with_url = file_to_with_url(&file, &storage);
            (StatusCode::OK, Json(with_url)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/site/files/{id}",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File soft-deleted"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, principal))]
pub async fn delete_file_handler(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.file.soft_delete(&id, &site.site_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))).into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "File deleted"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/site/files/{id}/references",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "References found", body = Vec<crate::models::file::FileReference>),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, principal))]
pub async fn get_file_references(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.file.get_references_for_site(&id, &site.site_id).await {
        Ok(refs) => (StatusCode::OK, Json(refs)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/files/{id}/restore",
    params(("id" = String, Path, description = "File ID")),
    responses(
        (status = 200, description = "File restored"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, principal))]
pub async fn restore_file(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.file.restore(&id, &site.site_id).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "File not found or not deleted"})),
        )
            .into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "File restored"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/files/batch-delete",
    request_body = BatchFileIds,
    responses(
        (status = 200, description = "Files soft-deleted"),
        (status = 400, description = "Bad request"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, principal, body))]
pub async fn batch_delete_files(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    match repository.file.batch_soft_delete(&site.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"deleted": count}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/files/batch-restore",
    request_body = BatchFileIds,
    responses(
        (status = 200, description = "Files restored"),
        (status = 400, description = "Bad request"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, principal, body))]
pub async fn batch_restore_files(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    match repository.file.batch_restore(&site.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"restored": count}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/files/batch-permanent-delete",
    request_body = BatchFileIds,
    responses(
        (status = 200, description = "Files permanently deleted"),
        (status = 400, description = "Bad request"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "files"
)]
#[instrument(skip(repository, storage, principal, body))]
pub async fn batch_permanent_delete_files(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
    Json(body): Json<BatchFileIds>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_ASSETS_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    if body.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No file IDs provided"}))).into_response();
    }

    let files = match repository.file.get_deleted_by_ids(&site.site_id, &body.ids).await {
        Ok(f) => f,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    for file in &files {
        let _ = remove_from_storage(&storage, &file.storage_provider, &file.storage_key).await;
        if let Some(ref tk) = file.thumbnail_key {
            let _ = remove_from_storage(&storage, &file.storage_provider, tk).await;
        }
    }

    match repository.file.batch_permanent_delete(&site.site_id, &body.ids).await {
        Ok(count) => (StatusCode::OK, Json(json!({"deleted": count}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[instrument(skip(repository, storage))]
pub async fn serve_file(
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    serve_file_by_key(&id, &repository, &storage, false).await
}

#[instrument(skip(repository, storage))]
pub async fn serve_file_thumbnail(
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    serve_file_by_key(&id, &repository, &storage, true).await
}

async fn serve_file_by_key(
    id: &str,
    repository: &Repository,
    storage: &StorageManager,
    use_thumbnail: bool,
) -> Response {
    let file = match repository.file.get_by_id_any(id).await {
        Ok(Some(f)) => f,
        Ok(None) => return (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    if file.deleted_at.is_some() {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    let (key, content_type) = if use_thumbnail {
        match &file.thumbnail_key {
            Some(tk) => {
                let mime = if tk.ends_with(".avif") {
                    "image/avif"
                } else if tk.ends_with(".webp") {
                    "image/webp"
                } else if tk.ends_with(".png") {
                    "image/png"
                } else {
                    "image/jpeg"
                };
                (tk.as_str(), mime)
            }
            None => return (StatusCode::NOT_FOUND, "No thumbnail").into_response(),
        }
    } else {
        (file.storage_key.as_str(), file.mime_type.as_str())
    };

    match read_from_storage(storage, &file.storage_provider, key).await {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
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
                    HeaderValue::from_str(&format!("inline; filename=\"{}\"", file.original_name))
                        .unwrap_or(HeaderValue::from_static("inline")),
                );
            }
            (StatusCode::OK, headers, Body::from(bytes)).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found in storage").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mime_to_ext_all_types() {
        let cases = [
            ("image/jpeg", "jpg"),
            ("image/jpg", "jpg"),
            ("image/png", "png"),
            ("image/gif", "gif"),
            ("image/webp", "webp"),
            ("image/avif", "avif"),
            ("image/svg+xml", "svg"),
            ("video/mp4", "mp4"),
            ("video/webm", "webm"),
            ("application/pdf", "pdf"),
            ("application/octet-stream", "bin"),
            ("text/plain", "bin"),
            ("", "bin"),
        ];
        for (mime, expected) in cases {
            assert_eq!(mime_to_ext(mime), expected, "mime_to_ext({mime:?})");
        }
    }

    #[test]
    fn test_storage_manager_flags_no_storage() {
        let sm = StorageManager {
            filesystem: None,
            s3: None,
        };
        assert!(!sm.has_s3());
        assert!(!sm.has_any());
    }

    #[test]
    fn test_storage_manager_flags_with_filesystem() {
        let dir = TempDir::new().unwrap();
        let sm = StorageManager {
            filesystem: Some(crate::storage::FileSystemStorage::new(dir.path().to_str().unwrap()).unwrap()),
            s3: None,
        };
        assert!(!sm.has_s3());
        assert!(sm.has_any());
    }
}
