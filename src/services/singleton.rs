use std::sync::Arc;

use axum::{Json, response::IntoResponse};
use serde_json::Value;
use serde_json::json;
use thiserror::Error;

use crate::handlers::file_handler::StorageManager;
use crate::models::collection::{Collection, SingletonResponse};
use crate::repository::Repository;

#[derive(Clone)]
pub struct SingletonService {
    repository: Arc<Repository>,
    storage: StorageManager,
}

#[derive(Error, Debug)]
pub enum SingletonError {
    #[error("Not found")]
    NotFound,

    #[error("Collection is not a singleton")]
    NotASingleton,

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
            SingletonError::DatabaseError(msg) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": msg})),
            ),
        };
        (status, body).into_response()
    }
}

impl SingletonService {
    pub fn new(repository: Arc<Repository>, storage: StorageManager) -> Self {
        Self { repository, storage }
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
            .repository
            .collection
            .list_singletons_only(site_id)
            .await
            .map_err(|e| SingletonError::DatabaseError(e.to_string()))?;

        Ok(items.iter().map(Self::collection_to_response).collect())
    }

    pub async fn get_singleton(&self, site_id: &str, slug: &str) -> Result<SingletonResponse, SingletonError> {
        let item = self
            .repository
            .collection
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| SingletonError::DatabaseError(e.to_string()))?
            .ok_or(SingletonError::NotFound)?;

        if !item.is_singleton {
            return Err(SingletonError::NotASingleton);
        }

        let mut response = Self::collection_to_response(&item);

        if let Some(ref data) = response.data {
            let resolved = self.resolve_files_from_value(data, &item.site_id).await;
            response.data = Some(resolved);
        }

        Ok(response)
    }

    pub async fn update_singleton(
        &self,
        site_id: &str,
        slug: &str,
        data: &Value,
    ) -> Result<SingletonResponse, SingletonError> {
        let item = self
            .repository
            .collection
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| SingletonError::DatabaseError(e.to_string()))?
            .ok_or(SingletonError::NotFound)?;

        if !item.is_singleton {
            return Err(SingletonError::NotASingleton);
        }

        let data_str = data.to_string();

        let updated = self
            .repository
            .collection
            .update_singleton_data(&item.id, &data_str)
            .await
            .map_err(|e| SingletonError::DatabaseError(e.to_string()))?;

        Ok(Self::collection_to_response(&updated))
    }

    async fn resolve_files_from_value(&self, data: &Value, site_id: &str) -> Value {
        let file_ids = self.extract_file_ids_from_value(data);

        let mut file_map = serde_json::Map::new();

        if !file_ids.is_empty() {
            if let Ok(file_items) = self.repository.file.get_by_ids(site_id, &file_ids).await {
                for f in file_items {
                    let url = match f.storage_provider.as_str() {
                        "s3" => self
                            .storage
                            .s3
                            .as_ref()
                            .map(|s| s.url(&f.storage_key))
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
