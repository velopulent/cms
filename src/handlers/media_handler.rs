use axum::{
    Json,
    body::Body,
    extract::{Extension, Path, Query},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use axum_extra::extract::multipart::Multipart;
use bytes::Bytes;
use image::ImageReader;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use std::io::Cursor;
use uuid::Uuid;

use crate::config::Config;
use crate::middleware::auth::AuthenticatedUser;
use crate::middleware::auth::check_site_access;
use crate::models::media::{Media, MediaReference, MediaWithUrl};
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
pub struct MediaListParams {
    pub page: Option<i64>,
    pub search: Option<String>,
    pub r#type: Option<String>,
}

fn media_to_with_url(media: &Media, storage: &StorageManager) -> MediaWithUrl {
    let url = match media.storage_provider.as_str() {
        "filesystem" => storage
            .filesystem
            .as_ref()
            .map(|s| s.url(&media.storage_key))
            .unwrap_or_else(|| format!("/media/{}/file", media.id)),
        "s3" => storage
            .s3
            .as_ref()
            .map(|s| s.url(&media.storage_key))
            .unwrap_or_else(|| format!("/media/{}/file", media.id)),
        _ => format!("/media/{}/file", media.id),
    };

    let thumbnail_url = media
        .thumbnail_key
        .as_ref()
        .map(|_| format!("/media/{}/thumbnail", media.id));

    MediaWithUrl {
        id: media.id.clone(),
        site_id: media.site_id.clone(),
        filename: media.filename.clone(),
        original_name: media.original_name.clone(),
        mime_type: media.mime_type.clone(),
        size: media.size,
        storage_provider: media.storage_provider.clone(),
        storage_key: media.storage_key.clone(),
        thumbnail_key: media.thumbnail_key.clone(),
        width: media.width,
        height: media.height,
        deleted_at: media.deleted_at.clone(),
        created_by: media.created_by.clone(),
        created_at: media.created_at.clone(),
        url,
        thumbnail_url,
    }
}

fn mime_to_ext(mime: &str) -> &str {
    match mime {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

fn generate_thumbnail(image_data: &[u8]) -> Option<Vec<u8>> {
    let reader = ImageReader::new(Cursor::new(image_data)).with_guessed_format().ok()?;
    let img = reader.decode().ok()?;
    let thumb = img.resize(300, 300, image::imageops::FilterType::Lanczos3);
    let mut bytes = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .ok()?;
    Some(bytes)
}

async fn store_file(
    storage: &StorageManager,
    provider: &str,
    key: &str,
    data: &[u8],
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

async fn get_file(
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

async fn delete_file(
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
    path = "/api/v1/sites/{site_id}/media",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        MediaListParams,
    ),
    responses(
        (status = 200, description = "List of media items"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "media"
)]
pub async fn list_media(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Query(params): Query<MediaListParams>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let page = params.page.unwrap_or(1).max(1);
    let per_page: i64 = 30;
    let offset = (page - 1) * per_page;

    let mut query = String::from(
        "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM media WHERE site_id = ? AND deleted_at IS NULL",
    );
    let mut count_query = String::from(
        "SELECT COUNT(*) FROM media WHERE site_id = ? AND deleted_at IS NULL",
    );

    let mut bindings: Vec<String> = vec![site_id.clone()];

    if let Some(search) = &params.search {
        query.push_str(" AND (original_name LIKE ? OR filename LIKE ?)");
        count_query.push_str(" AND (original_name LIKE ? OR filename LIKE ?)");
        let pattern = format!("%{}%", search);
        bindings.push(pattern.clone());
        bindings.push(pattern);
    }

    if let Some(type_filter) = &params.r#type {
        match type_filter.as_str() {
            "image" => {
                query.push_str(" AND mime_type LIKE 'image/%'");
                count_query.push_str(" AND mime_type LIKE 'image/%'");
            }
            "video" => {
                query.push_str(" AND mime_type LIKE 'video/%'");
                count_query.push_str(" AND mime_type LIKE 'video/%'");
            }
            "document" => {
                query.push_str(
                    " AND (mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/%' OR mime_type LIKE 'text/%')",
                );
                count_query.push_str(
                    " AND (mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/%' OR mime_type LIKE 'text/%')",
                );
            }
            _ => {}
        }
    }

    let count_bindings = bindings.clone();

    query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
    bindings.push(per_page.to_string());
    bindings.push(offset.to_string());

    // Fetch count
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_query);
    for b in &count_bindings {
        count_q = count_q.bind(b);
    }
    let total: i64 = count_q.fetch_optional(&pool).await.unwrap_or(Some(0)).unwrap_or(0);

    // Fetch items
    let mut q = sqlx::query_as::<_, Media>(&query);
    for b in &bindings {
        q = q.bind(b);
    }

    match q.fetch_all(&pool).await {
        Ok(items) => {
            let with_urls: Vec<MediaWithUrl> = items
                .into_iter()
                .map(|m| media_to_with_url(&m, &storage))
                .collect();

            (
                StatusCode::OK,
                Json(json!({
                    "items": with_urls,
                    "total": total,
                    "page": page,
                    "per_page": per_page,
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
    path = "/api/v1/sites/{site_id}/media",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 201, description = "Media uploaded", body = MediaWithUrl),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 413, description = "File too large"),
    ),
    security(("bearer" = [])),
    tag = "media"
)]
pub async fn upload_media(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
    Extension(config): Extension<Config>,
    mut multipart: Multipart,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let mut storage_provider = String::from("filesystem");
    let mut file_data: Option<Bytes> = None;
    let mut file_name: Option<String> = None;
    let mut file_content_type: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name: String = field.name().unwrap_or("").to_string();

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
                    let bytes: bytes::Bytes = bytes;
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
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No file provided"})),
            )
                .into_response()
        }
    };

    let original_name = file_name.unwrap_or_else(|| "upload".into());
    let content_type = file_content_type.unwrap_or_else(|| "application/octet-stream".into());

    let media_id = Uuid::now_v7().to_string();
    let ext = std::path::Path::new(&original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let filename = if ext.is_empty() {
        format!("{}.{}", &media_id[..8], mime_to_ext(&content_type))
    } else {
        format!("{}.{}", &media_id[..8], ext)
    };

    let storage_key = format!("{}/{}/{}", site_id, media_id, filename);
    let mime_type = content_type.clone();

    // Try to get image dimensions and generate thumbnail
    let mut width: Option<i32> = None;
    let mut height: Option<i32> = None;
    let mut thumbnail_data: Option<Vec<u8>> = None;
    let mut thumbnail_key: Option<String> = None;

    if mime_type.starts_with("image/") {
        if let Ok(reader) = ImageReader::new(Cursor::new(&file_data)).with_guessed_format() {
            if let Ok(img) = reader.decode() {
                width = Some(img.width() as i32);
                height = Some(img.height() as i32);

                if let Some(thumb_bytes) = generate_thumbnail(&file_data) {
                    let thumb_key =
                        format!("{}/{}/thumb_{}.jpg", site_id, media_id, &media_id[..8]);
                    thumbnail_data = Some(thumb_bytes);
                    thumbnail_key = Some(thumb_key);
                }
            }
        }
    }

    // Upload original file
    if let Err(e) = store_file(&storage, &storage_provider, &storage_key, &file_data, &mime_type).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store file: {}", e)})),
        )
            .into_response();
    }

    // Upload thumbnail
    if let (Some(thumb_data), Some(thumb_key)) = (&thumbnail_data, &thumbnail_key) {
        let _ = store_file(&storage, &storage_provider, thumb_key, thumb_data, "image/jpeg").await;
    }

    // Insert media record
    let thumb_key_str = thumbnail_key.clone();
    let result = sqlx::query(
        "INSERT INTO media (id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, created_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&media_id)
    .bind(&site_id)
    .bind(&filename)
    .bind(&original_name)
    .bind(&mime_type)
    .bind(file_data.len() as i64)
    .bind(&storage_provider)
    .bind(&storage_key)
    .bind(&thumb_key_str)
    .bind(width)
    .bind(height)
    .bind(&auth.user_id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let media = sqlx::query_as::<_, Media>(
                "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM media WHERE id = ?",
            )
            .bind(&media_id)
            .fetch_one(&pool)
            .await
            .unwrap();

            let with_url = media_to_with_url(&media, &storage);
            (StatusCode::CREATED, Json(with_url)).into_response()
        }
        Err(err) => {
            // Clean up uploaded files on DB error
            let _ = delete_file(&storage, &storage_provider, &storage_key).await;
            if let Some(ref tk) = thumbnail_key {
                let _ = delete_file(&storage, &storage_provider, tk).await;
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
    path = "/api/v1/sites/{site_id}/media/{id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Media ID"),
    ),
    responses(
        (status = 200, description = "Media item", body = MediaWithUrl),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
    tag = "media"
)]
pub async fn get_media(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    match sqlx::query_as::<_, Media>(
        "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM media WHERE id = ? AND site_id = ?",
    )
    .bind(&id)
    .bind(&site_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(media)) => {
            let with_url = media_to_with_url(&media, &storage);
            (StatusCode::OK, Json(with_url)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Media not found"})),
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
    path = "/api/v1/sites/{site_id}/media/{id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Media ID"),
    ),
    responses(
        (status = 200, description = "Media soft-deleted"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
    tag = "media"
)]
pub async fn delete_media(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query(
        "UPDATE media SET deleted_at = datetime('now') WHERE id = ? AND site_id = ? AND deleted_at IS NULL",
    )
    .bind(&id)
    .bind(&site_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Media not found"})),
        )
            .into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Media deleted"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/media/{id}/references",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Media ID"),
    ),
    responses(
        (status = 200, description = "References found", body = Vec<MediaReference>),
    ),
    security(("bearer" = [])),
    tag = "media"
)]
pub async fn get_media_references(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let pattern = format!("%media://{}%", id);

    match sqlx::query_as::<_, (String, String)>(
        "SELECT c.id, col.name FROM content c JOIN collections col ON c.collection_id = col.id WHERE c.site_id = ? AND c.data LIKE ?",
    )
    .bind(&site_id)
    .bind(&pattern)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => {
            let refs: Vec<MediaReference> = rows
                .into_iter()
                .map(|(content_id, collection_name)| MediaReference {
                    content_id,
                    collection_name,
                    field_name: String::new(),
                })
                .collect();
            (StatusCode::OK, Json(refs)).into_response()
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
    path = "/api/v1/sites/{site_id}/media/{id}/restore",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "Media ID"),
    ),
    responses(
        (status = 200, description = "Media restored"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
    tag = "media"
)]
pub async fn restore_media(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query(
        "UPDATE media SET deleted_at = NULL WHERE id = ? AND site_id = ? AND deleted_at IS NOT NULL",
    )
    .bind(&id)
    .bind(&site_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Media not found or not deleted"})),
        )
            .into_response(),
        Ok(_) => (StatusCode::OK, Json(json!({"message": "Media restored"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn serve_media_file(
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    serve_media_by_key(&id, &pool, &storage, false).await
}

pub async fn serve_media_thumbnail(
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    serve_media_by_key(&id, &pool, &storage, true).await
}

async fn serve_media_by_key(
    id: &str,
    pool: &SqlitePool,
    storage: &StorageManager,
    use_thumbnail: bool,
) -> Response {
    let media = match sqlx::query_as::<_, Media>(
        "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM media WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(m)) => m,
        Ok(None) => return (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // Don't serve soft-deleted media
    if media.deleted_at.is_some() {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    let (key, content_type) = if use_thumbnail {
        match &media.thumbnail_key {
            Some(tk) => (tk.as_str(), "image/jpeg"),
            None => return (StatusCode::NOT_FOUND, "No thumbnail").into_response(),
        }
    } else {
        (media.storage_key.as_str(), media.mime_type.as_str())
    };

    match get_file(storage, &media.storage_provider, key).await {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(content_type)
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            if !use_thumbnail {
                headers.insert(
                    header::CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!(
                        "inline; filename=\"{}\"",
                        media.original_name
                    ))
                    .unwrap_or(HeaderValue::from_static("inline")),
                );
            }
            (StatusCode::OK, headers, Body::from(bytes)).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found in storage").into_response(),
    }
}
