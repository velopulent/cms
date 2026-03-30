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
use sqlx::SqlitePool;
use std::io::Cursor;
use std::io::Write;
use tempfile::NamedTempFile;
use uuid::Uuid;

use crate::config::Config;
use crate::middleware::auth::AuthenticatedUser;
use crate::middleware::auth::check_site_access;
use crate::models::file::{File, FileReference, FileWithUrl};
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
    let thumb = img.resize(200, 200, image::imageops::FilterType::Lanczos3);
    let rgba = thumb.to_rgba8();
    let mut bytes = Vec::new();
    let encoder = image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut bytes, 7, 55);
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

/// Store a file directly from a filesystem path without loading it fully into memory.
/// For "filesystem" provider this uses a direct path copy.
/// For "s3" provider this reads the file to Bytes (S3 requires it).
async fn store_file_from_path(
    storage: &StorageManager,
    provider: &str,
    key: &str,
    path: &std::path::Path,
    content_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(
        "[store_file_from_path] provider={}, key={}, path={:?}, content_type={}",
        provider, key, path, content_type
    );
    let result = match provider {
        "filesystem" => {
            if let Some(fs) = &storage.filesystem {
                eprintln!("[store_file_from_path] Calling filesystem put_from_path...");
                fs.put_from_path(path, key, content_type).await
            } else {
                Err("Filesystem storage not available".into())
            }
        }
        "s3" => {
            if let Some(s3) = &storage.s3 {
                eprintln!("[store_file_from_path] Calling S3 put_from_path...");
                s3.put_from_path(path, key, content_type).await
            } else {
                Err("S3 storage not available".into())
            }
        }
        _ => Err(format!("Unknown storage provider: {}", provider).into()),
    };
    match &result {
        Ok(_) => eprintln!("[store_file_from_path] Success"),
        Err(e) => eprintln!("[store_file_from_path] Error: {}", e),
    }
    result
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
    path = "/api/v1/sites/{site_id}/files",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        FileListParams,
    ),
    responses(
        (status = 200, description = "List of files"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "files"
)]
pub async fn list_files(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Query(params): Query<FileListParams>,
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
        "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE site_id = ? AND deleted_at IS NULL",
    );
    let mut count_query =
        String::from("SELECT COUNT(*) FROM files WHERE site_id = ? AND deleted_at IS NULL");

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
    let total: i64 = count_q
        .fetch_optional(&pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    // Fetch items
    let mut q = sqlx::query_as::<_, File>(&query);
    for b in &bindings {
        q = q.bind(b);
    }

    match q.fetch_all(&pool).await {
        Ok(items) => {
            let with_urls: Vec<FileWithUrl> = items
                .into_iter()
                .map(|f| file_to_with_url(&f, &storage))
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
    path = "/api/v1/sites/{site_id}/files",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 201, description = "File uploaded", body = FileWithUrl),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 413, description = "File too large"),
    ),
    security(("bearer" = [])),
    tag = "files"
)]
pub async fn upload_file(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
    Extension(config): Extension<Config>,
    mut multipart: Multipart,
) -> Response {
    eprintln!("[upload_file] === Handler invoked ===");
    eprintln!(
        "[upload_file] Config max_upload_size_bytes: {} ({}MB)",
        config.max_upload_size_bytes,
        config.max_upload_size_bytes / (1024 * 1024)
    );

    // 1. Auth check — verify the user has editor access to this site
    eprintln!(
        "[upload_file] Step 1: Checking site access for user={} site={}",
        auth.user_id, site_id
    );
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        eprintln!("[upload_file] Access denied: {:?}", err);
        return (status, Json(err)).into_response();
    }
    eprintln!("[upload_file] Access OK");

    // 2. Determine default storage provider for this site
    eprintln!(
        "[upload_file] Step 2: Resolving storage provider for site={}",
        site_id
    );
    let site_storage =
        sqlx::query_scalar::<_, String>("SELECT default_storage_provider FROM sites WHERE id = ?")
            .bind(&site_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(Some("filesystem".into()))
            .unwrap_or("filesystem".into());

    let mut storage_provider = site_storage;
    eprintln!(
        "[upload_file] Default storage_provider={}",
        storage_provider
    );
    let mut temp_file: Option<NamedTempFile> = None;
    let mut file_name: Option<String> = None;
    let mut file_content_type: Option<String> = None;
    let mut file_size: i64 = 0;

    // 3. Parse multipart fields — stream the file to a temp file on disk
    eprintln!("[upload_file] Step 3: Parsing multipart fields");
    while let Ok(Some(field)) = multipart.next_field().await {
        let name: String = field.name().unwrap_or("").to_string();
        eprintln!("[upload_file]   Multipart field: name={}", name);

        if name == "storage_provider" {
            if let Ok(val) = field.text().await {
                eprintln!("[upload_file]   storage_provider field value={}", val);
                if val == "s3" && storage.has_s3() {
                    storage_provider = val;
                    eprintln!("[upload_file]   Switched to S3 storage");
                }
            }
        } else if name == "file" {
            file_name = field.file_name().map(String::from);
            file_content_type = field.content_type().map(String::from);
            eprintln!(
                "[upload_file]   File field: name={:?}, content_type={:?}",
                file_name, file_content_type
            );

            // Create a secure temporary file that auto-deletes on drop
            let mut tmp = match NamedTempFile::new() {
                Ok(f) => {
                    eprintln!(
                        "[upload_file]   Temp file created: {:?} (persisted={:?})",
                        f.path(),
                        f.as_file().metadata().map(|m| m.len()).unwrap_or(0)
                    );
                    f
                }
                Err(e) => {
                    eprintln!("[upload_file]   ERROR: Failed to create temp file: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to create temp file: {}", e)})),
                    )
                        .into_response();
                }
            };

            // NOTE: field.chunk() is broken in multer 3.1.0 (see rwf2/multer#67).
            // Using field.bytes() which works reliably, then writing to temp file.
            // The temp file still enables efficient store_file_from_path for filesystem storage.
            let max_bytes = config.max_upload_size_bytes;

            eprintln!(
                "[upload_file]   Reading file via field.bytes() (max={}MB)",
                max_bytes / (1024 * 1024)
            );
            match field.bytes().await {
                Ok(bytes) => {
                    eprintln!(
                        "[upload_file]   Read {} bytes from multipart field",
                        bytes.len()
                    );
                    if bytes.len() > max_bytes {
                        eprintln!(
                            "[upload_file]   PAYLOAD_TOO_LARGE: {} > {}",
                            bytes.len(),
                            max_bytes
                        );
                        return (
                            StatusCode::PAYLOAD_TOO_LARGE,
                            Json(json!({
                                "error": format!(
                                    "File too large. Maximum size is {}MB",
                                    max_bytes / (1024 * 1024)
                                )
                            })),
                        )
                            .into_response();
                    }
                    if let Err(e) = tmp.write_all(&bytes) {
                        eprintln!("[upload_file]   ERROR: write_all failed: {}", e);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Failed to write temp file: {}", e)})),
                        )
                            .into_response();
                    }
                    file_size = bytes.len() as i64;
                }
                Err(e) => {
                    eprintln!("[upload_file]   ERROR: field.bytes() failed: {}", e);
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Failed to read file: {}", e)})),
                    )
                        .into_response();
                }
            }

            // Flush writes to the OS
            eprintln!("[upload_file]   Flushing temp file...");
            if let Err(e) = tmp.flush() {
                eprintln!("[upload_file]   ERROR: flush failed: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to flush temp file: {}", e)})),
                )
                    .into_response();
            }

            // Verify the temp file size on disk
            if let Ok(meta) = std::fs::metadata(tmp.path()) {
                eprintln!(
                    "[upload_file]   Temp file on disk: {} bytes at {:?}",
                    meta.len(),
                    tmp.path()
                );
            }

            temp_file = Some(tmp);
            eprintln!("[upload_file]   file_size set to {}", file_size);
        } else {
            eprintln!("[upload_file]   Skipping unknown field: {}", name);
        }
    }
    eprintln!("[upload_file] Multipart parsing done");

    // 4. Validate we received a file
    let tmp = match temp_file {
        Some(f) => f,
        None => {
            eprintln!("[upload_file] ERROR: No file field found in multipart");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No file provided"})),
            )
                .into_response();
        }
    };

    let original_name = file_name.unwrap_or_else(|| "upload".into());
    let content_type = file_content_type.unwrap_or_else(|| "application/octet-stream".into());
    eprintln!(
        "[upload_file] original_name={}, content_type={}, size={}",
        original_name, content_type, file_size
    );

    // 5. Generate file metadata before storage operations
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

    let storage_key = format!("{}/{}/{}", site_id, file_id, filename);
    let mime_type = content_type.clone();
    eprintln!(
        "[upload_file] file_id={}, filename={}, storage_key={}",
        file_id, filename, storage_key
    );

    // 6. Image processing — read temp file once only for images
    let mut width: Option<i32> = None;
    let mut height: Option<i32> = None;
    let mut thumbnail_data: Option<(Vec<u8>, String)> = None;
    let mut thumbnail_key: Option<String> = None;

    if mime_type.starts_with("image/") {
        eprintln!("[upload_file] Step 6: Image detected, processing...");
        // Read the temp file into memory once for image decoding
        match tokio::fs::read(tmp.path()).await {
            Ok(img_bytes) => {
                eprintln!(
                    "[upload_file]   Read {} bytes from temp file for image processing",
                    img_bytes.len()
                );
                if let Ok(reader) = ImageReader::new(Cursor::new(&img_bytes)).with_guessed_format()
                {
                    if let Ok(img) = reader.decode() {
                        width = Some(img.width() as i32);
                        height = Some(img.height() as i32);
                        eprintln!(
                            "[upload_file]   Image dimensions: {}x{}",
                            img.width(),
                            img.height()
                        );

                        if let Some((thumb_bytes, thumb_mime)) = generate_thumbnail(&img) {
                            let thumb_key =
                                format!("{}/{}/thumb_{}.avif", site_id, file_id, &file_id[..8]);
                            eprintln!(
                                "[upload_file]   Thumbnail generated: {} bytes, mime={}, key={}",
                                thumb_bytes.len(),
                                thumb_mime,
                                thumb_key
                            );
                            thumbnail_data = Some((thumb_bytes, thumb_mime));
                            thumbnail_key = Some(thumb_key);
                        } else {
                            eprintln!("[upload_file]   Thumbnail generation returned None");
                        }
                    } else {
                        eprintln!("[upload_file]   WARNING: Failed to decode image");
                    }
                } else {
                    eprintln!("[upload_file]   WARNING: Failed to guess image format");
                }
                // img_bytes is dropped here — memory freed before storage upload
                eprintln!("[upload_file]   Image bytes freed from memory");
            }
            Err(e) => {
                eprintln!(
                    "[upload_file]   WARNING: Failed to read temp file for image processing: {}",
                    e
                );
            }
        }
    } else {
        eprintln!(
            "[upload_file] Step 6: Non-image file ({}), skipping image processing",
            mime_type
        );
    }

    // 7. Upload original file from temp file path
    eprintln!(
        "[upload_file] Step 7: Storing file with provider={} key={}",
        storage_provider, storage_key
    );
    if let Err(e) = store_file_from_path(
        &storage,
        &storage_provider,
        &storage_key,
        tmp.path(),
        &mime_type,
    )
    .await
    {
        eprintln!("[upload_file]   ERROR: store_file_from_path failed: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store file: {}", e)})),
        )
            .into_response();
    }
    eprintln!("[upload_file]   File stored successfully");

    // 8. Upload thumbnail (small, already in memory as Vec<u8>)
    if let (Some((thumb_data, thumb_mime)), Some(thumb_key)) = (&thumbnail_data, &thumbnail_key) {
        eprintln!(
            "[upload_file] Step 8: Storing thumbnail key={} ({} bytes)",
            thumb_key,
            thumb_data.len()
        );
        let _ = store_file(
            &storage,
            &storage_provider,
            thumb_key,
            Bytes::from(thumb_data.clone()),
            thumb_mime,
        )
        .await;
    } else {
        eprintln!("[upload_file] Step 8: No thumbnail to store");
    }

    // 9. Insert file record into database
    eprintln!("[upload_file] Step 9: Inserting DB record");
    let thumb_key_str = thumbnail_key.clone();
    let result = sqlx::query(
        "INSERT INTO files (id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, created_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&file_id)
    .bind(&site_id)
    .bind(&filename)
    .bind(&original_name)
    .bind(&mime_type)
    .bind(file_size)
    .bind(&storage_provider)
    .bind(&storage_key)
    .bind(&thumb_key_str)
    .bind(width)
    .bind(height)
    .bind(&auth.user_id)
    .execute(&pool)
    .await;

    // NamedTempFile drops here, cleaning up the temp file automatically
    eprintln!("[upload_file] Temp file cleaned up (NamedTempFile dropped)");

    match result {
        Ok(_) => {
            eprintln!("[upload_file] DB insert OK, fetching record...");
            let file = sqlx::query_as::<_, File>(
                "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE id = ?",
            )
            .bind(&file_id)
            .fetch_one(&pool)
            .await
            .unwrap();

            let with_url = file_to_with_url(&file, &storage);
            eprintln!("[upload_file] === Upload complete (201 CREATED) ===");
            (StatusCode::CREATED, Json(with_url)).into_response()
        }
        Err(err) => {
            eprintln!("[upload_file] ERROR: DB insert failed: {}", err);
            // Clean up uploaded files on DB insert failure
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
    path = "/api/v1/sites/{site_id}/files/{id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "File ID"),
    ),
    responses(
        (status = 200, description = "File item", body = FileWithUrl),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
    tag = "files"
)]
pub async fn get_file(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    match sqlx::query_as::<_, File>(
        "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE id = ? AND site_id = ?",
    )
    .bind(&id)
    .bind(&site_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(file)) => {
            let with_url = file_to_with_url(&file, &storage);
            (StatusCode::OK, Json(with_url)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "File not found"})),
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
    path = "/api/v1/sites/{site_id}/files/{id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "File ID"),
    ),
    responses(
        (status = 200, description = "File soft-deleted"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
    tag = "files"
)]
pub async fn delete_file_handler(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query(
        "UPDATE files SET deleted_at = datetime('now') WHERE id = ? AND site_id = ? AND deleted_at IS NULL",
    )
    .bind(&id)
    .bind(&site_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "File not found"})),
        )
            .into_response(),
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
    path = "/api/v1/sites/{site_id}/files/{id}/references",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "File ID"),
    ),
    responses(
        (status = 200, description = "References found", body = Vec<FileReference>),
    ),
    security(("bearer" = [])),
    tag = "files"
)]
pub async fn get_file_references(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    match sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT c.id, col.name FROM content_file_references cfr JOIN content c ON cfr.content_id = c.id JOIN collections col ON c.collection_id = col.id WHERE cfr.file_id = ?",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => {
            let refs: Vec<FileReference> = rows
                .into_iter()
                .map(|(content_id, collection_name)| FileReference {
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
    path = "/api/v1/sites/{site_id}/files/{id}/restore",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("id" = String, Path, description = "File ID"),
    ),
    responses(
        (status = 200, description = "File restored"),
        (status = 404, description = "Not found"),
    ),
    security(("bearer" = [])),
    tag = "files"
)]
pub async fn restore_file(
    auth: AuthenticatedUser,
    Path((site_id, id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query(
        "UPDATE files SET deleted_at = NULL WHERE id = ? AND site_id = ? AND deleted_at IS NOT NULL",
    )
    .bind(&id)
    .bind(&site_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
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

pub async fn serve_file(
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    serve_file_by_key(&id, &pool, &storage, false).await
}

pub async fn serve_file_thumbnail(
    Path(id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    serve_file_by_key(&id, &pool, &storage, true).await
}

async fn serve_file_by_key(
    id: &str,
    pool: &SqlitePool,
    storage: &StorageManager,
    use_thumbnail: bool,
) -> Response {
    let file = match sqlx::query_as::<_, File>(
        "SELECT id, site_id, filename, original_name, mime_type, size, storage_provider, storage_key, thumbnail_key, width, height, deleted_at, created_by, created_at FROM files WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(f)) => f,
        Ok(None) => return (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    // Don't serve soft-deleted files
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
                HeaderValue::from_str(content_type)
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            if !use_thumbnail {
                headers.insert(
                    header::CONTENT_DISPOSITION,
                    HeaderValue::from_str(&format!("inline; filename=\"{}\"", file.original_name))
                        .unwrap_or(HeaderValue::from_static("inline")),
                );
            } else {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
            }
            (StatusCode::OK, headers, Body::from(bytes)).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found in storage").into_response(),
    }
}
