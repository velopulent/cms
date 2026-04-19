use std::sync::Arc;

use axum::{http::StatusCode, Json, response::IntoResponse};
use bytes::Bytes;
use image::{DynamicImage, ImageEncoder, ImageReader};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::config::Config;
use crate::models::file::{File, FileReference, FileWithUrl};
use crate::repository::traits::{FileListResult, FileRepository, ListFilesParams};
use crate::storage::StorageProvider;

#[derive(Clone)]
pub struct FileService {
    file_repo: Arc<dyn FileRepository>,
    config: Arc<Config>,
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

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("No storage configured")]
    NoStorageConfigured,

    #[error("Database error: {0}")]
    DatabaseError(String),
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
        };
        (status, body).into_response()
    }
}

impl FileService {
    pub fn new(file_repo: Arc<dyn FileRepository>, config: Arc<Config>) -> Self {
        Self {
            file_repo,
            config,
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

    pub async fn upload_file(
        &self,
        site_id: &str,
        data: Bytes,
        filename: &str,
        content_type: &str,
        created_by: Option<&str>,
        storage: Arc<dyn StorageProvider>,
        storage_provider: &str,
    ) -> Result<FileWithUrl, FileError> {
        if data.len() as u64 > self.config.max_upload_size_bytes as u64 {
            return Err(FileError::FileTooLarge(format!(
                "File too large. Maximum size is {}MB",
                self.config.max_upload_size_bytes / (1024 * 1024)
            )));
        }

        let original_name = filename.to_string();
        let file_size = data.len() as i64;
        let file_id = Uuid::now_v7().to_string();
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

        let mut width: Option<i32> = None;
        let mut height: Option<i32> = None;
        let mut thumbnail_data: Option<(Vec<u8>, String)> = None;
        let mut thumbnail_key: Option<String> = None;

        if mime_type.starts_with("image/") {
            let data_clone = data.clone();
            let file_id_owned = file_id.clone();
            let site_id_owned = site_id.to_string();

            let result = tokio::task::spawn_blocking(move || {
                let mut w: Option<i32> = None;
                let mut h: Option<i32> = None;
                let mut tdata: Option<(Vec<u8>, String)> = None;
                let mut tkey: Option<String> = None;

                if let Ok(reader) = ImageReader::new(std::io::Cursor::new(&data_clone)).with_guessed_format() {
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

        storage
            .put(&storage_key, data.clone(), &mime_type)
            .await
            .map_err(|e| FileError::StorageError(e.to_string()))?;

        if let (Some((thumb_data, thumb_mime)), Some(thumb_key)) = (&thumbnail_data, &thumbnail_key) {
            let _ = storage
                .put(thumb_key, Bytes::from(thumb_data.clone()), thumb_mime)
                .await;
        }

        let thumb_key_str = thumbnail_key.as_deref();

        let file = self
            .file_repo
            .create(
                &file_id,
                site_id,
                &generated_filename,
                &original_name,
                &mime_type,
                file_size,
                storage_provider,
                &storage_key,
                thumb_key_str,
                width,
                height,
                created_by,
            )
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))?;

        Ok(self.file_to_with_url(&file, &*storage))
    }

    pub async fn soft_delete(&self, id: &str, site_id: &str) -> Result<u64, FileError> {
        self.file_repo
            .soft_delete(id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn restore(&self, id: &str, site_id: &str) -> Result<u64, FileError> {
        self.file_repo
            .restore(id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn batch_soft_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        self.file_repo
            .batch_soft_delete(site_id, ids)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn batch_restore(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        self.file_repo
            .batch_restore(site_id, ids)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn batch_permanent_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        self.file_repo
            .batch_permanent_delete(site_id, ids)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
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
        let file = self
            .file_repo
            .get_by_id_any(id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))?
            .ok_or(FileError::NotFound)?;

        if file.deleted_at.is_some() {
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
                    (tk.as_str(), mime)
                }
                None => return Err(FileError::NotFound),
            }
        } else {
            (file.storage_key.as_str(), file.mime_type.as_str())
        };

        let bytes = storage
            .get(key)
            .await
            .map_err(|e| FileError::StorageError(e.to_string()))?;

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
    use crate::test_helpers::InMemoryFileRepository;
    use crate::storage::MockStorage;
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
            .upload_file("site-123", data, "test.txt", "text/plain", None, storage, "filesystem")
            .await;

        assert!(matches!(result, Err(FileError::FileTooLarge(_))));
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

        let result = service
            .batch_soft_delete("site-123", &["file-123".to_string()])
            .await;
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

        let result = service
            .batch_restore("site-123", &["file-123".to_string()])
            .await;
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