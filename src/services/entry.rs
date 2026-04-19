use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::Value;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::models::entry::Entry;
use crate::repository::error::RepositoryError;
use crate::repository::traits::{EntriesListResult, EntryRepository, FileRepository, ListEntriesParams};
use crate::storage::StorageProvider;

#[derive(Clone)]
pub struct EntryService {
    entry_repo: Arc<dyn EntryRepository>,
    file_repo: Arc<dyn FileRepository>,
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
    pub fn new(entry_repo: Arc<dyn EntryRepository>, file_repo: Arc<dyn FileRepository>) -> Self {
        Self { entry_repo, file_repo }
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

        let _ = self.entry_repo.sync_file_references(id, site_id, &resolved_data).await;

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

    pub async fn resolve_entry_files(
        &self,
        entry: &Entry,
        storage: Arc<dyn StorageProvider>,
    ) -> Result<Value, EntryError> {
        let data: Value = serde_json::from_str(&entry.data).unwrap_or_default();
        let resolved_data = self.resolve_files_from_value(&data, &entry.site_id, storage).await;

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

    async fn resolve_files_from_value(&self, data: &Value, site_id: &str, storage: Arc<dyn StorageProvider>) -> Value {
        let file_ids = self.extract_file_ids_from_value(data);

        let mut file_map = serde_json::Map::new();

        if !file_ids.is_empty() {
            if let Ok(file_items) = self.file_repo.get_by_ids(site_id, &file_ids).await {
                for f in file_items {
                    let url = storage.url(&f.storage_key, &f.id);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MockStorage;
    use crate::test_helpers::{InMemoryEntryRepository, InMemoryFileRepository};
    use std::sync::Arc;

    fn test_entry_repo() -> Arc<InMemoryEntryRepository> {
        Arc::new(InMemoryEntryRepository::new())
    }

    fn test_file_repo() -> Arc<InMemoryFileRepository> {
        Arc::new(InMemoryFileRepository::new())
    }

    fn create_test_entry() -> Entry {
        Entry {
            id: "entry-123".to_string(),
            site_id: "site-123".to_string(),
            collection_id: "col-123".to_string(),
            data: r#"{"title": "Test Entry"}"#.to_string(),
            slug: "test-entry".to_string(),
            status: "draft".to_string(),
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
            published_at: None,
        }
    }

    #[tokio::test]
    async fn test_list_entries() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let params = ListEntriesParams {
            site_id: "site-123",
            collection_slug: None,
            collection_id: None,
            status: None,
            search: None,
            published_only: false,
            page: 1,
            per_page: 20,
        };

        let result = service.list_entries(params).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service.get_entry("nonexistent", "site-123", false).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_create_entry_success() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let data = json!({"title": "New Entry"});
        let result = service.create_entry("site-123", "col-123", &data, "new-entry").await;
        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.slug, "new-entry");
        assert_eq!(entry.status, "draft");
    }

    #[tokio::test]
    async fn test_create_entry_empty_data() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let data = json!({});
        let result = service.create_entry("site-123", "col-123", &data, "empty-entry").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_entry_success() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let new_data = json!({"title": "Updated Title"});
        let result = service
            .update_entry("entry-123", "site-123", Some(&new_data), Some("updated-slug"), None)
            .await;
        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.slug, "updated-slug");
    }

    #[tokio::test]
    async fn test_update_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service
            .update_entry("nonexistent", "site-123", Some(&json!({})), None, None)
            .await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_entry_status_only() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service
            .update_entry("entry-123", "site-123", None, None, Some("published"))
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, "published");
    }

    #[tokio::test]
    async fn test_delete_entry_success() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service.delete_entry("entry-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_delete_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service.delete_entry("nonexistent", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_publish_entry_success() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service.publish_entry("entry-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, "published");
    }

    #[tokio::test]
    async fn test_publish_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service.publish_entry("nonexistent", "site-123").await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_unpublish_entry_success() {
        let entry_repo = test_entry_repo();
        let mut entry = create_test_entry();
        entry.status = "published".to_string();
        entry_repo.add_entry(entry);
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service.unpublish_entry("entry-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, "draft");
    }

    #[tokio::test]
    async fn test_unpublish_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let result = service.unpublish_entry("nonexistent", "site-123").await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_resolve_entry_files_with_no_files() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let entry = create_test_entry();
        let storage = Arc::new(MockStorage::default());
        let result = service.resolve_entry_files(&entry, storage).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_entries_list_files() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let entries = vec![create_test_entry()];
        let result = service.resolve_entries_list_files(&entries).await;
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_extract_file_ids_from_value() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let data = json!({
            "title": "Test",
            "body": "/api/files/abc-123-def/image.jpg"
        });
        let ids = service.extract_file_ids_from_value(&data);
        assert_eq!(ids, vec!["abc-123-def"]);
    }

    #[test]
    fn test_extract_file_ids_multiple() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let data = json!({
            "images": ["/api/files/abc-123-def/image.png", "/api/files/456-789-abc/image.png"]
        });
        let ids = service.extract_file_ids_from_value(&data);
        assert_eq!(ids, vec!["abc-123-def", "456-789-abc"]);
    }

    #[test]
    fn test_extract_file_ids_no_matches() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let data = json!({"title": "No files here"});
        let ids = service.extract_file_ids_from_value(&data);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_extract_file_ids_invalid_format() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo);

        let data = json!({"url": "/api/files/not-a-uuid"});
        let ids = service.extract_file_ids_from_value(&data);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_entry_error_into_response() {
        assert_eq!(
            EntryError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            EntryError::AlreadyExists.into_response().status(),
            axum::http::StatusCode::CONFLICT
        );
        assert_eq!(
            EntryError::DatabaseError("bad".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
