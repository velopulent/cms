use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use bytes::Bytes;
use image::{DynamicImage, ImageEncoder, ImageReader};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::config::Config;
use crate::handlers::file_handler::StorageManager;
use crate::models::file::{File, FileReference, FileWithUrl};
use crate::repository::Repository;
use crate::repository::traits::{FileListResult, ListFilesParams};

#[derive(Clone)]
pub struct FileService {
    repository: Arc<Repository>,
    storage: StorageManager,
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
            FileError::NotFound => {
                (StatusCode::NOT_FOUND, Json(json!({"error": "File not found"})))
            }
            FileError::NotFoundOrNotDeleted => {
                (StatusCode::NOT_FOUND, Json(json!({"error": "File not found or not deleted"})))
            }
            FileError::NoFileProvided => {
                (StatusCode::BAD_REQUEST, Json(json!({"error": "No file provided"})))
            }
            FileError::FileTooLarge(msg) => {
                (StatusCode::PAYLOAD_TOO_LARGE, Json(json!({"error": msg})))
            }
            FileError::StorageError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to store file: {}", msg)})))
            }
            FileError::NoStorageConfigured => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "No storage providers configured"})))
            }
            FileError::DatabaseError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
            }
        };
        (status, body).into_response()
    }
}

impl FileService {
    pub fn new(repository: Arc<Repository>, storage: StorageManager, config: Arc<Config>) -> Self {
        Self {
            repository,
            storage,
            config,
        }
    }

    pub async fn list_files(
        &self,
        params: ListFilesParams<'_>,
    ) -> Result<FileListResult, FileError> {
        self.repository
            .file
            .list(params)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn get_file(&self, id: &str, site_id: &str) -> Result<Option<File>, FileError> {
        self.repository
            .file
            .get_by_id(id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn get_file_any(&self, id: &str) -> Result<Option<File>, FileError> {
        self.repository
            .file
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
        storage_provider: Option<&str>,
        created_by: Option<&str>,
    ) -> Result<FileWithUrl, FileError> {
        if data.len() as u64 > self.config.max_upload_size_bytes as u64 {
            return Err(FileError::FileTooLarge(format!(
                "File too large. Maximum size is {}MB",
                self.config.max_upload_size_bytes / (1024 * 1024)
            )));
        }

        let mut provider = storage_provider.unwrap_or("filesystem");
        if provider == "s3" && !self.storage.has_s3() {
            provider = "filesystem";
        }

        if provider == "filesystem" && self.storage.filesystem.is_none() {
            return Err(FileError::NoStorageConfigured);
        }
        if provider == "s3" && self.storage.s3.is_none() {
            return Err(FileError::NoStorageConfigured);
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

        self.store_file(&storage_key, data.clone(), &mime_type, provider)
            .await
            .map_err(|e| FileError::StorageError(e.to_string()))?;

        if let (Some((thumb_data, thumb_mime)), Some(thumb_key)) = (&thumbnail_data, &thumbnail_key) {
            let _ = self
                .store_file(thumb_key, Bytes::from(thumb_data.clone()), thumb_mime, provider)
                .await;
        }

        let thumb_key_str = thumbnail_key.as_deref();

        let file = self
            .repository
            .file
            .create(
                &file_id,
                site_id,
                &generated_filename,
                &original_name,
                &mime_type,
                file_size,
                provider,
                &storage_key,
                thumb_key_str,
                width,
                height,
                created_by,
            )
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))?;

        Ok(self.file_to_with_url(&file))
    }

    pub async fn soft_delete(&self, id: &str, site_id: &str) -> Result<u64, FileError> {
        self.repository
            .file
            .soft_delete(id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn restore(&self, id: &str, site_id: &str) -> Result<u64, FileError> {
        self.repository
            .file
            .restore(id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn batch_soft_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        self.repository
            .file
            .batch_soft_delete(site_id, ids)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn batch_restore(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        self.repository
            .file
            .batch_restore(site_id, ids)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn batch_permanent_delete(&self, site_id: &str, ids: &[String]) -> Result<u64, FileError> {
        let files = self
            .repository
            .file
            .get_deleted_by_ids(site_id, ids)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))?;

        for file in &files {
            let _ = self.remove_from_storage(&file.storage_key, &file.storage_provider).await;
            if let Some(ref tk) = file.thumbnail_key {
                let _ = self.remove_from_storage(tk, &file.storage_provider).await;
            }
        }

        self.repository
            .file
            .batch_permanent_delete(site_id, ids)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn get_file_references(&self, file_id: &str, site_id: &str) -> Result<Vec<FileReference>, FileError> {
        self.repository
            .file
            .get_references_for_site(file_id, site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn get_storage_provider(&self, site_id: &str) -> Result<String, FileError> {
        self.repository
            .file
            .get_storage_provider(site_id)
            .await
            .map_err(|e| FileError::DatabaseError(e.to_string()))
    }

    pub async fn serve_file(&self, id: &str, use_thumbnail: bool) -> Result<(Bytes, String, String), FileError> {
        let file = self
            .repository
            .file
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

        let bytes = self
            .read_from_storage(key, &file.storage_provider)
            .await
            .map_err(|e| FileError::StorageError(e.to_string()))?;

        Ok((bytes, content_type.to_string(), file.original_name))
    }

    pub(crate) fn file_to_with_url(&self, file: &File) -> FileWithUrl {
        let url = match file.storage_provider.as_str() {
            "s3" => self
                .storage
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

    async fn store_file(
        &self,
        key: &str,
        data: Bytes,
        content_type: &str,
        provider: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match provider {
            "filesystem" => {
                if let Some(fs) = &self.storage.filesystem {
                    fs.put(key, data, content_type).await
                } else {
                    Err("Filesystem storage not available".into())
                }
            }
            "s3" => {
                if let Some(s3) = &self.storage.s3 {
                    s3.put(key, data, content_type).await
                } else {
                    Err("S3 storage not available".into())
                }
            }
            _ => Err(format!("Unknown storage provider: {}", provider).into()),
        }
    }

    async fn read_from_storage(
        &self,
        key: &str,
        provider: &str,
    ) -> Result<Bytes, Box<dyn std::error::Error>> {
        match provider {
            "filesystem" => {
                if let Some(fs) = &self.storage.filesystem {
                    fs.get(key).await
                } else {
                    Err("Filesystem storage not available".into())
                }
            }
            "s3" => {
                if let Some(s3) = &self.storage.s3 {
                    s3.get(key).await
                } else {
                    Err("S3 storage not available".into())
                }
            }
            _ => Err(format!("Unknown storage provider: {}", provider).into()),
        }
    }

    async fn remove_from_storage(
        &self,
        key: &str,
        provider: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match provider {
            "filesystem" => {
                if let Some(fs) = &self.storage.filesystem {
                    fs.delete(key).await
                } else {
                    Err("Filesystem storage not available".into())
                }
            }
            "s3" => {
                if let Some(s3) = &self.storage.s3 {
                    s3.delete(key).await
                } else {
                    Err("S3 storage not available".into())
                }
            }
            _ => Err(format!("Unknown storage provider: {}", provider).into()),
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
