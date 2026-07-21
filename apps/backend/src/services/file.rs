use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use bytes::Bytes;
use dashmap::DashMap;
use futures_util::{Stream, StreamExt};
use image::{DynamicImage, ImageEncoder, ImageReader};
use object_store::WriteMultipart;
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::models::file::{File, FileReference, FileWithUrl};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{FileListResult, FileRepository, ListFilesParams, NewFile};
use crate::storage::StorageProvider;
use crate::utils::magic_bytes::{self, SNIFF_LEN, Sniff};

/// Multipart part size for streaming uploads. S3 requires >= 5 MiB for every
/// part but the last; with ~2 parts in flight, worst case is ~15 MiB of memory
/// per upload regardless of file size.
const MULTIPART_CHUNK_SIZE: usize = 5 * 1024 * 1024;
/// Max multipart parts concurrently in flight per upload.
const MULTIPART_MAX_CONCURRENCY: usize = 2;

#[derive(Clone)]
pub struct FileService {
    file_repo: Arc<dyn FileRepository>,
    config: Arc<Config>,
    /// Pre-generated file ids currently being uploaded (signed-URL flow); makes
    /// one-time-use race-free in-process. The `files` PK is the durable backstop.
    in_flight: Arc<DashMap<String, ()>>,
}

/// Inputs for [`FileService::upload_file`].
pub struct UploadFileRequest<'a> {
    pub site_id: &'a str,
    pub data: Bytes,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub created_by: Option<&'a str>,
    pub storage: Arc<dyn StorageProvider>,
    pub storage_provider: &'a str,
}

/// Inputs for [`FileService::upload_file_streaming`]. `file_id` is set by the
/// signed-URL flow (the token pre-generates it, enforcing one-time use).
pub struct StreamingUploadRequest<'a> {
    pub site_id: &'a str,
    pub file_id: Option<&'a str>,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub created_by: Option<&'a str>,
    pub storage: Arc<dyn StorageProvider>,
    pub storage_provider: &'a str,
    pub max_bytes: usize,
}

/// RAII claim on an in-flight pre-generated file id; released on drop.
struct InFlightClaim {
    map: Arc<DashMap<String, ()>>,
    key: String,
}

impl InFlightClaim {
    fn acquire(map: &Arc<DashMap<String, ()>>, key: &str) -> Option<Self> {
        use dashmap::mapref::entry::Entry;
        match map.entry(key.to_string()) {
            Entry::Occupied(_) => None,
            Entry::Vacant(v) => {
                v.insert(());
                Some(Self {
                    map: map.clone(),
                    key: key.to_string(),
                })
            }
        }
    }
}

impl Drop for InFlightClaim {
    fn drop(&mut self) {
        self.map.remove(&self.key);
    }
}

#[derive(Error, Debug)]
pub enum FileError {
    #[error("Not found")]
    NotFound,

    #[error("File not found or not deleted")]
    NotFoundOrNotDeleted,

    #[error("No file provided")]
    NoFileProvided,

    #[error("File too large: {0}")]
    FileTooLarge(String),

    #[error("Invalid content type: {0}")]
    InvalidContentType(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("No storage configured")]
    NoStorageConfigured,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("File already exists")]
    AlreadyExists,

    #[error("Failed to read upload body: {0}")]
    ReadError(String),
}

impl FileError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            FileError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"}))),
            FileError::NotFoundOrNotDeleted => (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "File not found or not deleted"})),
            ),
            FileError::NoFileProvided => (StatusCode::BAD_REQUEST, Json(json!({"error": "No file provided"}))),
            FileError::FileTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, Json(json!({"error": msg}))),
            FileError::StorageError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to store file: {}", msg)})),
            ),
            FileError::NoStorageConfigured => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "No storage providers configured"})),
            ),
            FileError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))),
            FileError::InvalidContentType(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            FileError::AlreadyExists => (
                StatusCode::CONFLICT,
                Json(json!({"error": "File already exists (upload URL already used)"})),
            ),
            FileError::ReadError(msg) => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Failed to read upload body: {}", msg)})),
            ),
        };
        (status, body).into_response()
    }
}

impl FileService {
    pub fn new(file_repo: Arc<dyn FileRepository>, config: Arc<Config>) -> Self {
        Self {
            file_repo,
            config,
            in_flight: Arc::new(DashMap::new()),
        }
    }

    pub async fn list_files(&self, params: ListFilesParams<'_>) -> Result<FileListResult, FileError> {
        self.file_repo
            .list(params)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn get_file(&self, id: &str, site_id: &str) -> Result<Option<File>, FileError> {
        self.file_repo
            .get_by_id(id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn get_file_any(&self, id: &str) -> Result<Option<File>, FileError> {
        self.file_repo
            .get_by_id_any(id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    /// Buffered upload — thin wrapper over [`Self::upload_file_streaming`] for
    /// callers that already hold the whole payload (gRPC, tests).
    pub async fn upload_file(&self, req: UploadFileRequest<'_>) -> Result<FileWithUrl, FileError> {
        let UploadFileRequest {
            site_id,
            data,
            filename,
            content_type,
            created_by,
            storage,
            storage_provider,
        } = req;

        let stream = futures_util::stream::iter(std::iter::once(Ok(data)));
        self.upload_file_streaming(
            StreamingUploadRequest {
                site_id,
                file_id: None,
                filename,
                content_type,
                created_by,
                storage,
                storage_provider,
                max_bytes: self.config.max_upload_size_bytes,
            },
            stream,
        )
        .await
    }

    /// Streaming upload: constant memory regardless of file size. Enforces the
    /// content-type whitelist, magic-byte sniffing on the first bytes, and the
    /// size cap while streaming; aborts the multipart upload on any failure so
    /// no partial object is left behind. Thumbnails are generated in a
    /// background task after the DB record exists.
    pub async fn upload_file_streaming<S>(
        &self,
        req: StreamingUploadRequest<'_>,
        mut stream: S,
    ) -> Result<FileWithUrl, FileError>
    where
        S: Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Unpin + Send,
    {
        let StreamingUploadRequest {
            site_id,
            file_id,
            filename,
            content_type,
            created_by,
            storage,
            storage_provider,
            max_bytes,
        } = req;

        info!(
            "Uploading file (streaming): site_id={}, content_type={}",
            site_id, content_type
        );

        if !crate::utils::content_types::is_allowed(content_type) {
            return Err(FileError::InvalidContentType(format!(
                "Content type '{}' is not allowed. Accepted types: images, videos, audio, documents, archives",
                content_type
            )));
        }

        // One-time-use for pre-generated ids (signed URLs): claim the id
        // in-process, then verify no record exists. The PK insert below is the
        // durable backstop.
        let _claim = match file_id {
            Some(id) => {
                let claim = InFlightClaim::acquire(&self.in_flight, id).ok_or(FileError::AlreadyExists)?;
                let existing = self
                    .file_repo
                    .get_by_id_any(id)
                    .await
                    .map_err(|e| FileError::DatabaseError(e.to_string()))?;
                if existing.is_some() {
                    return Err(FileError::AlreadyExists);
                }
                Some(claim)
            }
            None => None,
        };

        let file_id = file_id
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::now_v7().to_string());
        let original_name = filename.to_string();
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let generated_filename = if ext.is_empty() {
            format!("{}.{}", &file_id[..8], self.mime_to_ext(content_type))
        } else {
            format!("{}.{}", &file_id[..8], ext)
        };
        let storage_key = format!("s_{}/f_{}/{}", site_id, file_id, generated_filename);
        let mime_type = content_type.to_string();
        let max = max_bytes;

        let upload = storage.start_multipart(&storage_key).await.map_err(|e| {
            error!("Failed to start multipart upload: key={}, error={}", storage_key, e);
            FileError::StorageError(e.to_string())
        })?;
        let mut writer = WriteMultipart::new_with_chunk_size(upload, MULTIPART_CHUNK_SIZE);

        let too_large =
            || FileError::FileTooLarge(format!("File too large. Maximum size is {}MB", max / (1024 * 1024)));

        // Hold back the first SNIFF_LEN bytes so nothing is written before the
        // magic-byte check passes; afterwards chunks stream straight through.
        let mut sniff_buf: Vec<u8> = Vec::new();
        let mut sniffed = false;
        let mut total: usize = 0;

        loop {
            let chunk = match stream.next().await {
                Some(Ok(chunk)) => chunk,
                Some(Err(e)) => {
                    let _ = writer.abort().await;
                    return Err(FileError::ReadError(e.to_string()));
                }
                None => break,
            };

            total += chunk.len();
            if total > max {
                let _ = writer.abort().await;
                warn!("File too large: site_id={}, size>{} bytes, max={}", site_id, total, max);
                return Err(too_large());
            }

            if !sniffed {
                sniff_buf.extend_from_slice(&chunk);
                if sniff_buf.len() >= SNIFF_LEN {
                    if let Sniff::Mismatch(detected) = magic_bytes::check(&mime_type, &sniff_buf) {
                        let _ = writer.abort().await;
                        return Err(content_mismatch(&mime_type, detected));
                    }
                    sniffed = true;
                    if let Err(e) = writer.wait_for_capacity(MULTIPART_MAX_CONCURRENCY).await {
                        let _ = writer.abort().await;
                        return Err(FileError::StorageError(e.to_string()));
                    }
                    writer.write(&sniff_buf);
                    sniff_buf = Vec::new();
                }
            } else {
                if let Err(e) = writer.wait_for_capacity(MULTIPART_MAX_CONCURRENCY).await {
                    let _ = writer.abort().await;
                    return Err(FileError::StorageError(e.to_string()));
                }
                writer.write(&chunk);
            }
        }

        // EOF before the sniff buffer filled: check whatever we have.
        if !sniffed {
            if let Sniff::Mismatch(detected) = magic_bytes::check(&mime_type, &sniff_buf) {
                let _ = writer.abort().await;
                return Err(content_mismatch(&mime_type, detected));
            }
            if !sniff_buf.is_empty() {
                writer.write(&sniff_buf);
            }
        }

        if let Err(e) = writer.finish().await {
            error!("Failed to finish multipart upload: key={}, error={}", storage_key, e);
            return Err(FileError::StorageError(e.to_string()));
        }
        debug!("File stored successfully: key={}, size={} bytes", storage_key, total);

        let file = match self
            .file_repo
            .create(NewFile {
                id: &file_id,
                site_id,
                filename: &generated_filename,
                original_name: &original_name,
                mime_type: &mime_type,
                size: total as i64,
                storage_provider,
                storage_key: &storage_key,
                thumbnail_key: None,
                width: None,
                height: None,
                created_by,
            })
            .await
        {
            Ok(file) => file,
            Err(RepositoryError::UniqueViolation(_)) => {
                // A concurrent upload with the same pre-generated id won the
                // insert. Do NOT delete the blob — the winner shares the key.
                warn!("Duplicate file id on insert (upload URL reused): id={}", file_id);
                return Err(FileError::AlreadyExists);
            }
            Err(e) => {
                error!("Failed to create file record: id={}, error={}", file_id, e);
                // Orphan cleanup: the blob was written but has no DB record.
                if let Err(del_err) = storage.delete(&storage_key).await {
                    warn!(
                        "Failed to clean up orphaned blob: key={}, error={}",
                        storage_key, del_err
                    );
                }
                return Err(FileError::DatabaseError(e.to_string()));
            }
        };

        info!(
            "File uploaded successfully: id={}, site_id={}, size={} bytes",
            file.id, site_id, file.size
        );

        if mime_type.starts_with("image/") {
            self.spawn_thumbnail_task(file.clone(), storage.clone());
        }

        Ok(self.file_to_with_url(&file, &*storage))
    }

    /// Generate thumbnail + dimensions off the upload critical path: read the
    /// image back from storage, encode in a blocking task, update the record.
    /// Failures are logged and leave the thumbnail fields NULL.
    fn spawn_thumbnail_task(&self, file: File, storage: Arc<dyn StorageProvider>) {
        let repo = self.file_repo.clone();
        tokio::spawn(async move {
            let data = match storage.get(&file.storage_key).await {
                Ok(data) => data,
                Err(e) => {
                    warn!("Thumbnail task: failed to read {}: {}", file.storage_key, e);
                    return;
                }
            };

            let result = tokio::task::spawn_blocking(move || {
                let reader = ImageReader::new(std::io::Cursor::new(&data))
                    .with_guessed_format()
                    .ok()?;
                let img = reader.decode().ok()?;
                let (w, h) = (img.width() as i32, img.height() as i32);
                let thumb = generate_thumbnail(&img)?;
                Some((w, h, thumb))
            })
            .await;

            match result {
                Ok(Some((width, height, (thumb_bytes, thumb_mime)))) => {
                    let thumb_key = format!("s_{}/f_{}/thumb_{}.avif", file.site_id, file.id, &file.id[..8]);
                    if let Err(e) = storage.put(&thumb_key, Bytes::from(thumb_bytes), &thumb_mime).await {
                        warn!("Thumbnail task: failed to store {}: {}", thumb_key, e);
                        return;
                    }
                    if let Err(e) = repo
                        .set_thumbnail_meta(&file.id, &thumb_key, Some(width), Some(height))
                        .await
                    {
                        warn!("Thumbnail task: failed to update record {}: {}", file.id, e);
                    } else {
                        debug!("Thumbnail generated: id={}, key={}", file.id, thumb_key);
                    }
                }
                Ok(None) => debug!("Thumbnail task: {} not decodable as image", file.id),
                Err(e) => warn!("Thumbnail task panicked for {}: {}", file.id, e),
            }
        });
    }

    pub async fn soft_delete(&self, id: &str, site_id: &str) -> Result<u64, FileError> {
        info!("Soft deleting file");

        match self.file_repo.soft_delete(id, site_id).await {
            Ok(deleted_count) => {
                info!("File soft deleted successfully: deleted_count={}", deleted_count);
                Ok(deleted_count)
            }
            Err(e) => {
                error!("Failed to soft delete file: error={}", e);
                Err(FileError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn restore(&self, id: &str, site_id: &str) -> Result<u64, FileError> {
        info!("Restoring file");

        match self.file_repo.restore(id, site_id).await {
            Ok(restored_count) => {
                info!("File restored successfully: restored_count={}", restored_count);
                Ok(restored_count)
            }
            Err(e) => {
                error!("Failed to restore file: error={}", e);
                Err(FileError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn batch_soft_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        info!("Batch soft deleting files: site_id={}, count={}", site_id, ids.len());

        match self.file_repo.batch_soft_delete(site_id, ids).await {
            Ok(deleted_count) => {
                info!(
                    "Batch soft delete completed: site_id={}, requested={}, deleted={}",
                    site_id,
                    ids.len(),
                    deleted_count
                );
                Ok(deleted_count)
            }
            Err(e) => {
                error!("Failed to batch soft delete files: site_id={}, error={}", site_id, e);
                Err(FileError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn batch_restore(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        info!("Batch restoring files: site_id={}, count={}", site_id, ids.len());

        match self.file_repo.batch_restore(site_id, ids).await {
            Ok(restored_count) => {
                info!(
                    "Batch restore completed: site_id={}, requested={}, restored={}",
                    site_id,
                    ids.len(),
                    restored_count
                );
                Ok(restored_count)
            }
            Err(e) => {
                error!("Failed to batch restore files: site_id={}, error={}", site_id, e);
                Err(FileError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn batch_permanent_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        info!(
            "Batch permanently deleting files: site_id={}, count={}",
            site_id,
            ids.len()
        );

        match self.file_repo.batch_permanent_delete(site_id, ids).await {
            Ok(deleted_count) => {
                info!(
                    "Batch permanent delete completed: site_id={}, requested={}, deleted={}",
                    site_id,
                    ids.len(),
                    deleted_count
                );
                Ok(deleted_count)
            }
            Err(e) => {
                error!(
                    "Failed to batch permanently delete files: site_id={}, error={}",
                    site_id, e
                );
                Err(FileError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn get_file_references(&self, file_id: &str, site_id: &str) -> Result<Vec<FileReference>, FileError> {
        self.file_repo
            .get_references_for_site(file_id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn get_storage_provider(&self, site_id: &str) -> Result<String, FileError> {
        self.file_repo
            .get_storage_provider(site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn serve_file(
        &self,
        id: &str,
        use_thumbnail: bool,
        storage: Arc<dyn StorageProvider>,
    ) -> Result<(Bytes, String, String), FileError> {
        debug!("Serving file: id={}, use_thumbnail={}", id, use_thumbnail);

        let file = self
            .file_repo
            .get_by_id_any(id)
            .await
            .map_err(|e| {
                error!("Failed to fetch file metadata: id={}, error={}", id, e);
                FileError::DatabaseError(e.to_string())
            })?
            .ok_or(FileError::NotFound)?;

        if file.deleted_at.is_some() {
            warn!("Attempt to serve deleted file: id={}", id);
            return Err(FileError::NotFound);
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
                    debug!("Serving thumbnail: id={}, thumb_key={}, mime={}", id, tk, mime);
                    (tk.as_str(), mime)
                }
                None => {
                    warn!("Thumbnail requested but not available: id={}", id);
                    return Err(FileError::NotFound);
                }
            }
        } else {
            debug!(
                "Serving file: id={}, storage_key={}, mime={}",
                id, file.storage_key, file.mime_type
            );
            (file.storage_key.as_str(), file.mime_type.as_str())
        };

        let bytes = match storage.get(key).await {
            Ok(data) => {
                debug!(
                    "File served successfully: id={}, size={} bytes, storage_key={}",
                    id,
                    data.len(),
                    key
                );
                data
            }
            Err(e) => {
                error!(
                    "Failed to retrieve file from storage: id={}, key={}, error={}",
                    id, key, e
                );
                return Err(FileError::StorageError(e.to_string()));
            }
        };

        Ok((bytes, content_type.to_string(), file.original_name))
    }

    pub(crate) fn file_to_with_url(&self, file: &File, storage: &dyn StorageProvider) -> FileWithUrl {
        let url = storage.url(&file.storage_key, &file.id);

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

    fn mime_to_ext(&self, mime: &str) -> &str {
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
}

/// 400 error for a declared/detected content-type contradiction.
fn content_mismatch(declared: &str, detected: Option<&'static str>) -> FileError {
    FileError::InvalidContentType(match detected {
        Some(d) => format!(
            "File content does not match declared type '{}' (detected '{}')",
            declared, d
        ),
        None => format!("File content does not match declared type '{}'", declared),
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::storage::MockStorage;
    use crate::test_helpers::InMemoryFileRepository;
    use std::sync::Arc;

    fn test_config() -> Arc<Config> {
        Arc::new(Config {
            max_upload_size_bytes: 50 * 1024 * 1024,
            ..Default::default()
        })
    }

    fn test_file_repo() -> Arc<InMemoryFileRepository> {
        Arc::new(InMemoryFileRepository::new())
    }

    fn create_test_file() -> File {
        File {
            id: "file-123".to_string(),
            site_id: "site-123".to_string(),
            filename: "test.jpg".to_string(),
            original_name: "test.jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
            size: 1024,
            storage_provider: "filesystem".to_string(),
            storage_key: "s_site-123/f_file-123/test.jpg".to_string(),
            thumbnail_key: Some("s_site-123/f_file-123/thumb_abc.avif".to_string()),
            width: Some(800),
            height: Some(600),
            deleted_at: None,
            created_by: Some("user-123".to_string()),
            created_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    #[tokio::test]
    async fn test_list_files() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let params = ListFilesParams {
            site_id: "site-123",
            trashed: false,
            search: None,
            file_type: None,
            page: 1,
            per_page: 20,
        };

        let result = service.list_files(params).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_file_not_found() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.get_file("nonexistent", "site-123").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_file_any_not_found() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.get_file_any("nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_upload_file_file_too_large() {
        let file_repo = test_file_repo();
        let config = Arc::new(Config {
            max_upload_size_bytes: 100,
            ..Default::default()
        });
        let service = FileService::new(file_repo, config);

        let storage = Arc::new(MockStorage::default());
        let data = Bytes::from(&[0u8; 200][..]);

        let result = service
            .upload_file(UploadFileRequest {
                site_id: "site-123",
                data,
                filename: "test.txt",
                content_type: "text/plain",
                created_by: None,
                storage,
                storage_provider: "filesystem",
            })
            .await;

        assert!(matches!(result, Err(FileError::FileTooLarge(_))));
    }

    #[tokio::test]
    async fn test_upload_file_streams_to_storage_and_creates_record() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo.clone(), config);

        let storage = Arc::new(MockStorage::default());
        let result = service
            .upload_file(UploadFileRequest {
                site_id: "site-123",
                data: Bytes::from("hello world"),
                filename: "notes.txt",
                content_type: "text/plain",
                created_by: None,
                storage: storage.clone(),
                storage_provider: "filesystem",
            })
            .await
            .expect("upload should succeed");

        assert_eq!(result.size, 11);
        assert!(result.thumbnail_url.is_none());
        let stored = storage.files.lock().unwrap();
        assert_eq!(stored.get(&result.storage_key).unwrap().as_ref(), b"hello world");
    }

    #[tokio::test]
    async fn test_upload_streaming_magic_byte_mismatch_rejected() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let storage = Arc::new(MockStorage::default());
        let result = service
            .upload_file(UploadFileRequest {
                site_id: "site-123",
                data: Bytes::from("definitely not a png"),
                filename: "fake.png",
                content_type: "image/png",
                created_by: None,
                storage: storage.clone(),
                storage_provider: "filesystem",
            })
            .await;

        assert!(matches!(result, Err(FileError::InvalidContentType(_))));
        // Aborted upload must leave nothing behind.
        assert!(storage.files.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_upload_streaming_pregenerated_id_single_use() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);
        let storage = Arc::new(MockStorage::default());

        let req = |storage: Arc<MockStorage>| StreamingUploadRequest {
            site_id: "site-123",
            file_id: Some("0197-fixed-id"),
            filename: "a.txt",
            content_type: "text/plain",
            created_by: None,
            storage,
            storage_provider: "filesystem",
            max_bytes: 1024,
        };

        let first = service
            .upload_file_streaming(
                req(storage.clone()),
                futures_util::stream::iter(std::iter::once(Ok(Bytes::from("one")))),
            )
            .await;
        assert!(first.is_ok());

        let second = service
            .upload_file_streaming(
                req(storage.clone()),
                futures_util::stream::iter(std::iter::once(Ok(Bytes::from("two")))),
            )
            .await;
        assert!(matches!(second, Err(FileError::AlreadyExists)));
    }

    #[tokio::test]
    async fn test_soft_delete_success() {
        let file_repo = test_file_repo();
        file_repo.add_file(create_test_file());
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.soft_delete("file-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_soft_delete_not_found() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.soft_delete("nonexistent", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_restore_success() {
        let file_repo = test_file_repo();
        let mut file = create_test_file();
        file.deleted_at = Some("2024-01-01 00:00:00".to_string());
        file_repo.add_file(file);
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.restore("file-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_restore_not_found() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.restore("nonexistent", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_batch_soft_delete() {
        let file_repo = test_file_repo();
        file_repo.add_file(create_test_file());
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.batch_soft_delete("site-123", &["file-123".to_string()]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_batch_restore() {
        let file_repo = test_file_repo();
        let mut file = create_test_file();
        file.deleted_at = Some("2024-01-01 00:00:00".to_string());
        file_repo.add_file(file);
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.batch_restore("site-123", &["file-123".to_string()]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_batch_permanent_delete() {
        let file_repo = test_file_repo();
        file_repo.add_file(create_test_file());
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service
            .batch_permanent_delete("site-123", &["file-123".to_string()])
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_get_file_references() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.get_file_references("file-123", "site-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_storage_provider() {
        let file_repo = test_file_repo();
        file_repo.add_file(create_test_file());
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let result = service.get_storage_provider("site-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_serve_file_not_found() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let storage = Arc::new(MockStorage::default());
        let result = service.serve_file("nonexistent", false, storage).await;
        assert!(matches!(result, Err(FileError::NotFound)));
    }

    #[tokio::test]
    async fn test_serve_file_deleted() {
        let file_repo = test_file_repo();
        let mut file = create_test_file();
        file.deleted_at = Some("2024-01-01 00:00:00".to_string());
        file_repo.add_file(file);
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let storage = Arc::new(MockStorage::default());
        let result = service.serve_file("file-123", false, storage).await;
        assert!(matches!(result, Err(FileError::NotFound)));
    }

    #[tokio::test]
    async fn test_serve_file_thumbnail_not_found() {
        let file_repo = test_file_repo();
        let mut file = create_test_file();
        file.thumbnail_key = None;
        file_repo.add_file(file);
        let config = test_config();
        let service = FileService::new(file_repo, config);

        let storage = Arc::new(MockStorage::default());
        let result = service.serve_file("file-123", true, storage).await;
        assert!(matches!(result, Err(FileError::NotFound)));
    }

    #[test]
    fn test_mime_to_ext_jpeg() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("image/jpeg"), "jpg");
        assert_eq!(service.mime_to_ext("image/jpg"), "jpg");
    }

    #[test]
    fn test_mime_to_ext_png() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("image/png"), "png");
    }

    #[test]
    fn test_mime_to_ext_gif() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("image/gif"), "gif");
    }

    #[test]
    fn test_mime_to_ext_webp() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("image/webp"), "webp");
    }

    #[test]
    fn test_mime_to_ext_avif() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("image/avif"), "avif");
    }

    #[test]
    fn test_mime_to_ext_svg() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("image/svg+xml"), "svg");
    }

    #[test]
    fn test_mime_to_ext_video() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("video/mp4"), "mp4");
        assert_eq!(service.mime_to_ext("video/webm"), "webm");
    }

    #[test]
    fn test_mime_to_ext_pdf() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("application/pdf"), "pdf");
    }

    #[test]
    fn test_mime_to_ext_unknown() {
        let file_repo = test_file_repo();
        let config = test_config();
        let service = FileService::new(file_repo, config);

        assert_eq!(service.mime_to_ext("application/octet-stream"), "bin");
        assert_eq!(service.mime_to_ext("text/plain"), "bin");
    }

    #[test]
    fn test_file_error_into_response() {
        assert_eq!(
            FileError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            FileError::NotFoundOrNotDeleted.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            FileError::NoFileProvided.into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            FileError::FileTooLarge("too large".into()).into_response().status(),
            axum::http::StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            FileError::StorageError("fail".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            FileError::NoStorageConfigured.into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            FileError::DatabaseError("bad".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
