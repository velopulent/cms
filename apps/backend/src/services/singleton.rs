use std::sync::Arc;

use axum::{Json, response::IntoResponse};
use serde_json::Value;
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::models::collection::{Collection, SingletonResponse};
use crate::repository::traits::{CollectionRepository, FileRepository};
use crate::storage::StorageProvider;

#[derive(Clone)]
pub struct SingletonService {
    collection_repo: Arc<dyn CollectionRepository>,
    file_repo: Arc<dyn FileRepository>,
}

#[derive(Error, Debug)]
pub enum SingletonError {
    #[error("Not found")]
    NotFound,

    #[error("Collection is not a singleton")]
    NotASingleton,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl SingletonError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            SingletonError::NotFound => (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({"error": "Singleton not found"})),
            ),
            SingletonError::NotASingleton => (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({"error": "Singleton not found"})),
            ),
            SingletonError::ValidationFailed(msg) => (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({"error": msg})),
            ),
            SingletonError::DatabaseError(msg) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": msg})),
            ),
        };
        (status, body).into_response()
    }
}

impl SingletonService {
    pub fn new(collection_repo: Arc<dyn CollectionRepository>, file_repo: Arc<dyn FileRepository>) -> Self {
        Self {
            collection_repo,
            file_repo,
        }
    }

    fn collection_to_response(c: &Collection) -> SingletonResponse {
        let definition: Value = serde_json::from_str(&c.definition).unwrap_or(json!({"fields": []}));
        let data = c.singleton_data.as_ref().and_then(|d| serde_json::from_str(d).ok());

        SingletonResponse {
            id: c.id.clone(),
            site_id: c.site_id.clone(),
            name: c.name.clone(),
            slug: c.slug.clone(),
            definition,
            data,
            created_at: c.created_at.clone(),
            updated_at: c.updated_at.clone(),
        }
    }

    pub async fn list_singletons(&self, site_id: &str) -> Result<Vec<SingletonResponse>, SingletonError> {
        let items = self
            .collection_repo
            .list_singletons_only(site_id)
            .await
            .map_err(|e| SingletonError::DatabaseError(e.to_string()))?;

        Ok(items.iter().map(Self::collection_to_response).collect())
    }

    pub async fn get_singleton(
        &self,
        site_id: &str,
        slug: &str,
        storage: Arc<dyn StorageProvider>,
    ) -> Result<SingletonResponse, SingletonError> {
        debug!("Fetching singleton: site_id={}, slug={}", site_id, slug);

        let item = self
            .collection_repo
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch singleton from repository: site_id={}, slug={}, error={}",
                    site_id, slug, e
                );
                SingletonError::DatabaseError(e.to_string())
            })?
            .ok_or(SingletonError::NotFound)?;

        debug!(
            "Fetched collection item: id={}, is_singleton={}",
            item.id, item.is_singleton
        );

        if !item.is_singleton {
            warn!(
                "Collection is not a singleton: id={}, site_id={}, slug={}",
                item.id, site_id, slug
            );
            return Err(SingletonError::NotASingleton);
        }

        debug!("Collection is a singleton, building response: id={}", item.id);
        let mut response = Self::collection_to_response(&item);

        if let Some(ref data) = response.data {
            debug!("Resolving file references in singleton data");
            let resolved = self.resolve_files_from_value(data, &item.site_id, storage).await;
            response.data = Some(resolved);
        }

        info!(
            "Singleton retrieved successfully: id={}, site_id={}, slug={}",
            item.id, site_id, slug
        );
        Ok(response)
    }

    pub async fn update_singleton(
        &self,
        site_id: &str,
        slug: &str,
        data: &Value,
    ) -> Result<SingletonResponse, SingletonError> {
        debug!("Updating singleton: site_id={}, slug={}", site_id, slug);

        let item = self
            .collection_repo
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch singleton for update: site_id={}, slug={}, error={}",
                    site_id, slug, e
                );
                SingletonError::DatabaseError(e.to_string())
            })?
            .ok_or(SingletonError::NotFound)?;

        if !item.is_singleton {
            warn!(
                "Collection is not a singleton: id={}, site_id={}, slug={}",
                item.id, site_id, slug
            );
            return Err(SingletonError::NotASingleton);
        }

        // Validate singleton data against collection definition
        if let Ok(definition) = serde_json::from_str::<Value>(&item.definition)
            && let Some(fields) = definition.get("fields").and_then(|f| f.as_array())
                && let Some(err) =
                    super::definition_validation::validate_entry_data(data, fields)
                {
                    return Err(SingletonError::ValidationFailed(err));
                }

        let data_str = data.to_string();
        debug!("Updating singleton data for item: id={}", item.id);

        let updated = self
            .collection_repo
            .update_singleton_data(&item.id, &data_str)
            .await
            .map_err(|e| {
                error!(
                    "Failed to update singleton data in repository: id={}, error={}",
                    item.id, e
                );
                SingletonError::DatabaseError(e.to_string())
            })?;

        info!("Singleton updated successfully: id={}", item.id);
        Ok(Self::collection_to_response(&updated))
    }

    async fn resolve_files_from_value(&self, data: &Value, site_id: &str, storage: Arc<dyn StorageProvider>) -> Value {
        let file_ids = self.extract_file_ids_from_value(data);

        let mut file_map = serde_json::Map::new();

        if !file_ids.is_empty()
            && let Ok(file_items) = self.file_repo.get_by_ids(site_id, &file_ids).await {
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
    use crate::test_helpers::{InMemoryCollectionRepository, InMemoryFileRepository};
    use std::sync::Arc;

    fn test_collection_repo() -> Arc<InMemoryCollectionRepository> {
        Arc::new(InMemoryCollectionRepository::new())
    }

    fn test_file_repo() -> Arc<InMemoryFileRepository> {
        Arc::new(InMemoryFileRepository::new())
    }

    fn create_test_collection() -> Collection {
        Collection {
            id: "col-123".to_string(),
            site_id: "site-123".to_string(),
            name: "Test Singleton".to_string(),
            slug: "test-singleton".to_string(),
            definition: r#"{"fields": [{"name": "title", "type": "text"}]}"#.to_string(),
            is_singleton: true,
            singleton_data: Some(r#"{"title": "Hello"}"#.to_string()),
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    fn create_non_singleton_collection() -> Collection {
        Collection {
            id: "col-456".to_string(),
            site_id: "site-123".to_string(),
            name: "Regular Collection".to_string(),
            slug: "regular".to_string(),
            definition: r#"{"fields": []}"#.to_string(),
            is_singleton: false,
            singleton_data: None,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    #[tokio::test]
    async fn test_list_singletons() {
        let collection_repo = test_collection_repo();
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let result = service.list_singletons("site-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_singleton_not_found() {
        let collection_repo = test_collection_repo();
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let storage = Arc::new(MockStorage::default());
        let result = service.get_singleton("site-123", "nonexistent", storage).await;
        assert!(matches!(result, Err(SingletonError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_singleton_not_a_singleton() {
        let collection_repo = test_collection_repo();
        collection_repo.add_collection(create_non_singleton_collection());
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let storage = Arc::new(MockStorage::default());
        let result = service.get_singleton("site-123", "regular", storage).await;
        assert!(matches!(result, Err(SingletonError::NotASingleton)));
    }

    #[tokio::test]
    async fn test_get_singleton_success() {
        let collection_repo = test_collection_repo();
        collection_repo.add_collection(create_test_collection());
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let storage = Arc::new(MockStorage::default());
        let result = service.get_singleton("site-123", "test-singleton", storage).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.name, "Test Singleton");
        assert!(response.data.is_some());
    }

    #[tokio::test]
    async fn test_update_singleton_not_found() {
        let collection_repo = test_collection_repo();
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let result = service
            .update_singleton("site-123", "nonexistent", &json!({"title": "Updated"}))
            .await;
        assert!(matches!(result, Err(SingletonError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_singleton_not_a_singleton() {
        let collection_repo = test_collection_repo();
        collection_repo.add_collection(create_non_singleton_collection());
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let result = service
            .update_singleton("site-123", "regular", &json!({"title": "Updated"}))
            .await;
        assert!(matches!(result, Err(SingletonError::NotASingleton)));
    }

    #[tokio::test]
    async fn test_update_singleton_success() {
        let collection_repo = test_collection_repo();
        collection_repo.add_collection(create_test_collection());
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let result = service
            .update_singleton("site-123", "test-singleton", &json!({"title": "Updated Title"}))
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_file_ids() {
        let collection_repo = test_collection_repo();
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let data = json!({
            "title": "Test",
            "image": "/api/files/abc-123-def/test.jpg"
        });
        let ids = service.extract_file_ids_from_value(&data);
        assert_eq!(ids, vec!["abc-123-def"]);
    }

    #[test]
    fn test_extract_file_ids_multiple() {
        let collection_repo = test_collection_repo();
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let data = json!({
            "images": ["/api/files/abc-123-def/image.png", "/api/files/456-789-abc/image.png"]
        });
        let ids = service.extract_file_ids_from_value(&data);
        assert_eq!(ids, vec!["abc-123-def", "456-789-abc"]);
    }

    #[test]
    fn test_extract_file_ids_none() {
        let collection_repo = test_collection_repo();
        let file_repo = test_file_repo();
        let service = SingletonService::new(collection_repo, file_repo);

        let data = json!({"title": "No files here"});
        let ids = service.extract_file_ids_from_value(&data);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_singleton_error_into_response() {
        assert_eq!(
            SingletonError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            SingletonError::NotASingleton.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            SingletonError::DatabaseError("bad".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_collection_to_response() {
        let collection = create_test_collection();
        let response = SingletonService::collection_to_response(&collection);

        assert_eq!(response.id, "col-123");
        assert_eq!(response.name, "Test Singleton");
        assert_eq!(response.slug, "test-singleton");
        assert!(response.data.is_some());
    }
}
