use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::Value;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::handlers::file_handler::StorageManager;
use crate::models::entry::Entry;
use crate::repository::error::RepositoryError;
use crate::repository::traits::{EntriesListResult, EntryRepository, FileRepository, ListEntriesParams};

#[derive(Clone)]
pub struct EntryService {
    entry_repo: Arc<dyn EntryRepository>,
    file_repo: Arc<dyn FileRepository>,
    storage: StorageManager,
}

#[derive(Error, Debug)]
pub enum EntryError {
    #[error("Not found")]
    NotFound,

    #[error("Entry with this slug already exists for this collection")]
    AlreadyExists,

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl EntryError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            EntryError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))),
            EntryError::AlreadyExists => (
                StatusCode::CONFLICT,
                Json(json!({"error": "Entry with this slug already exists for this collection"})),
            ),
            EntryError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))),
        };
        (status, body).into_response()
    }
}

impl EntryService {
    pub fn new(entry_repo: Arc<dyn EntryRepository>, file_repo: Arc<dyn FileRepository>, storage: StorageManager) -> Self {
        Self { entry_repo, file_repo, storage }
    }

    pub async fn list_entries(&self, params: ListEntriesParams<'_>) -> Result<EntriesListResult, EntryError> {
        self.entry_repo
            .list(params)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))
    }

    pub async fn get_entry(&self, id: &str, site_id: &str, published_only: bool) -> Result<Option<Entry>, EntryError> {
        self.entry_repo
            .get_by_id(id, site_id, published_only)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))
    }

    pub async fn create_entry(
        &self,
        site_id: &str,
        collection_id: &str,
        data: &Value,
        slug: &str,
    ) -> Result<Entry, EntryError> {
        let id = Uuid::now_v7().to_string();
        let data_str = data.to_string();

        self.entry_repo
            .create(&id, site_id, collection_id, &data_str, slug)
            .await
            .map_err(|e| match e {
                RepositoryError::UniqueViolation(_) => EntryError::AlreadyExists,
                _ => EntryError::DatabaseError(e.to_string()),
            })?;

        let _ = self.entry_repo.sync_file_references(&id, site_id, data).await;

        self.entry_repo
            .get_by_id(&id, site_id, false)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))?
            .ok_or(EntryError::NotFound)
    }

    pub async fn update_entry(
        &self,
        id: &str,
        site_id: &str,
        data: Option<&Value>,
        slug: Option<&str>,
        status: Option<&str>,
    ) -> Result<Entry, EntryError> {
        let existing = self
            .entry_repo
            .get_by_id(id, site_id, false)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))?
            .ok_or(EntryError::NotFound)?;

        let resolved_data = match data {
            Some(d) => d.clone(),
            None => serde_json::from_str(&existing.data).unwrap_or(Value::Null),
        };
        let data_str = resolved_data.to_string();
        let slug = slug.unwrap_or(&existing.slug);
        let status = status.unwrap_or(&existing.status);

        self.entry_repo
            .update(id, &data_str, slug, status)
            .await
            .map_err(|e| match e {
                RepositoryError::UniqueViolation(_) => EntryError::AlreadyExists,
                _ => EntryError::DatabaseError(e.to_string()),
            })?;

        let _ = self
            .entry_repo
            .sync_file_references(id, site_id, &resolved_data)
            .await;

        self.entry_repo
            .get_by_id(id, site_id, false)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))?
            .ok_or(EntryError::NotFound)
    }

    pub async fn delete_entry(&self, id: &str, site_id: &str) -> Result<u64, EntryError> {
        self.entry_repo
            .delete(id, site_id)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))
    }

    pub async fn publish_entry(&self, id: &str, site_id: &str) -> Result<Entry, EntryError> {
        self.entry_repo.publish(id, site_id).await.map_err(|e| match e {
            RepositoryError::NotFound => EntryError::NotFound,
            _ => EntryError::DatabaseError(e.to_string()),
        })
    }

    pub async fn unpublish_entry(&self, id: &str, site_id: &str) -> Result<Entry, EntryError> {
        self.entry_repo.unpublish(id, site_id).await.map_err(|e| match e {
            RepositoryError::NotFound => EntryError::NotFound,
            _ => EntryError::DatabaseError(e.to_string()),
        })
    }

    pub async fn resolve_entry_files(&self, entry: &Entry) -> Result<Value, EntryError> {
        let data: Value = serde_json::from_str(&entry.data).unwrap_or_default();
        let resolved_data = self.resolve_files_from_value(&data, &entry.site_id).await;

        Ok(json!({
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
        }))
    }

    pub async fn resolve_entries_list_files(&self, items: &[Entry]) -> Vec<Entry> {
        items.to_vec()
    }

    async fn resolve_files_from_value(&self, data: &Value, site_id: &str) -> Value {
        let file_ids = self.extract_file_ids_from_value(data);

        let mut file_map = serde_json::Map::new();

        if !file_ids.is_empty() {
            if let Ok(file_items) = self.file_repo.get_by_ids(site_id, &file_ids).await {
                for f in file_items {
                    let url = match f.storage_provider.as_str() {
                        "s3" => self
                            .storage
                            .s3
                            .as_ref()
                            .map(|s| s.url(&f.storage_key, &f.id))
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

    fn extract_file_ids_from_value(&self, data: &Value) -> Vec<String> {
        let re = regex::Regex::new(r"/api/files/([a-f0-9-]+)").unwrap();
        let json_str = data.to_string();
        re.captures_iter(&json_str)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect()
    }
}
