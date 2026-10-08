use std::collections::HashMap;
use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::models::collection::Collection;
use crate::repository::error::RepositoryError;
use crate::repository::traits::CollectionRepository;

#[derive(Clone)]
pub struct CollectionService {
    collection_repo: Arc<dyn CollectionRepository>,
}

#[derive(Error, Debug)]
pub enum CollectionError {
    #[error("Not found")]
    NotFound,

    #[error("Collection with this name or slug already exists")]
    AlreadyExists,

    #[error("Schema changed while the update was being prepared")]
    PreconditionFailed,

    #[error("Invalid definition: {0}")]
    InvalidDefinition(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl CollectionError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            CollectionError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Collection not found"}))),
            CollectionError::AlreadyExists => (
                StatusCode::CONFLICT,
                Json(json!({"error": "Collection with this name or slug already exists"})),
            ),
            CollectionError::PreconditionFailed => (
                StatusCode::PRECONDITION_FAILED,
                Json(json!({"error":"Schema changed; refresh and retry"})),
            ),
            CollectionError::InvalidDefinition(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            CollectionError::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal_error", "message": "Internal server error"})),
            ),
        };
        (status, body).into_response()
    }
}

impl CollectionService {
    pub fn new(collection_repo: Arc<dyn CollectionRepository>) -> Self {
        Self { collection_repo }
    }

    pub async fn list_collections(&self, site_id: &str) -> Result<Vec<Collection>, CollectionError> {
        self.collection_repo
            .list(site_id)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn list_singletons_only(&self, site_id: &str) -> Result<Vec<Collection>, CollectionError> {
        self.collection_repo
            .list_singletons_only(site_id)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn get_collection(&self, site_id: &str, slug: &str) -> Result<Option<Collection>, CollectionError> {
        self.collection_repo
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<Collection>, CollectionError> {
        self.collection_repo
            .get_by_id(id)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn create_collection(
        &self,
        site_id: &str,
        name: &str,
        slug: &str,
        definition: &str,
        is_singleton: bool,
    ) -> Result<Collection, CollectionError> {
        let id = Uuid::now_v7().to_string();
        debug!(
            "Creating collection: site_id={}, name={}, slug={}, is_singleton={}",
            site_id, name, slug, is_singleton
        );

        let definition_value: serde_json::Value =
            serde_json::from_str(definition).unwrap_or(serde_json::json!({"fields": []}));
        let normalized = super::definition_validation::normalize_definition(&definition_value)
            .map_err(CollectionError::InvalidDefinition)?;
        let definition_str = normalized.to_string();

        self.collection_repo
            .create(&id, site_id, name, slug, &definition_str, is_singleton)
            .await
            .map_err(|e| {
                error!(
                    "Failed to create collection in repository: site_id={}, name={}, slug={}, error={}",
                    site_id, name, slug, e
                );
                match e {
                    RepositoryError::UniqueViolation(_) => CollectionError::AlreadyExists,
                    _ => CollectionError::DatabaseError(e.to_string()),
                }
            })?;

        info!(
            "Collection created successfully: id={}, site_id={}, name={}, slug={}",
            id, site_id, name, slug
        );

        self.collection_repo
            .get_by_id(&id)
            .await
            .map_err(|e| {
                error!("Failed to fetch created collection: id={}, error={}", id, e);
                CollectionError::DatabaseError(e.to_string())
            })?
            .ok_or(CollectionError::NotFound)
    }

    pub async fn update_collection(
        &self,
        site_id: &str,
        slug: &str,
        name: Option<&str>,
        new_slug: Option<&str>,
        definition: Option<&str>,
    ) -> Result<Collection, CollectionError> {
        debug!(
            "Updating collection: site_id={}, slug={}, name={:?}, new_slug={:?}, definition_provided={}",
            site_id,
            slug,
            name,
            new_slug,
            definition.is_some()
        );

        let existing = self
            .collection_repo
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch existing collection for update: site_id={}, slug={}, error={}",
                    site_id, slug, e
                );
                CollectionError::DatabaseError(e.to_string())
            })?
            .ok_or(CollectionError::NotFound)?;

        debug!(
            "Fetched existing collection: id={}, name={}, slug={}",
            existing.id, existing.name, existing.slug
        );

        let name = name.unwrap_or(&existing.name);
        let new_slug = new_slug.unwrap_or(&existing.slug);
        let definition_str = match definition {
            Some(d) => {
                let value: serde_json::Value = serde_json::from_str(d).unwrap_or(serde_json::json!({"fields": []}));
                let normalized = super::definition_validation::normalize_definition(&value)
                    .map_err(CollectionError::InvalidDefinition)?;
                normalized.to_string()
            }
            None => existing.definition.clone(),
        };

        let name_changed = name != existing.name;
        let slug_changed = new_slug != existing.slug;
        let definition_changed = definition.is_some();

        debug!(
            "Collection changes: name_changed={}, slug_changed={}, definition_changed={}",
            name_changed, slug_changed, definition_changed
        );

        let rename_map = if definition.is_some() {
            let old: serde_json::Value = serde_json::from_str(&existing.definition)
                .map_err(|error| CollectionError::DatabaseError(error.to_string()))?;
            let new: serde_json::Value = serde_json::from_str(&definition_str)
                .map_err(|error| CollectionError::InvalidDefinition(error.to_string()))?;
            compute_field_rename_map(&old, &new)
        } else {
            HashMap::new()
        };

        debug!("Updating collection in repository: id={}", existing.id);
        self.collection_repo
            .update(
                &existing.id,
                name,
                new_slug,
                &definition_str,
                &rename_map,
                &existing.definition,
            )
            .await
            .map_err(|e| {
                error!(
                    "Failed to update collection in repository: id={}, error={}",
                    existing.id, e
                );
                match e {
                    RepositoryError::UniqueViolation(_) => CollectionError::AlreadyExists,
                    RepositoryError::PreconditionFailed => CollectionError::PreconditionFailed,
                    RepositoryError::NotFound => CollectionError::NotFound,
                    _ => CollectionError::DatabaseError(e.to_string()),
                }
            })?;

        info!(
            "Collection updated successfully: id={}, site_id={}, name={}, slug={}",
            existing.id, site_id, name, new_slug
        );

        self.collection_repo
            .get_by_id(&existing.id)
            .await
            .map_err(|e| {
                error!("Failed to fetch updated collection: id={}, error={}", existing.id, e);
                CollectionError::DatabaseError(e.to_string())
            })?
            .ok_or(CollectionError::NotFound)
    }

    pub async fn delete_collection(&self, site_id: &str, slug: &str) -> Result<u64, CollectionError> {
        info!("Deleting collection: site_id={}, slug={}", site_id, slug);

        match self.collection_repo.delete(site_id, slug).await {
            Ok(deleted_count) => {
                info!(
                    "Collection deleted successfully: site_id={}, slug={}, deleted_count={}",
                    site_id, slug, deleted_count
                );
                Ok(deleted_count)
            }
            Err(e) => {
                error!(
                    "Failed to delete collection: site_id={}, slug={}, error={}",
                    site_id, slug, e
                );
                Err(CollectionError::DatabaseError(e.to_string()))
            }
        }
    }
}

pub fn compute_field_rename_map(old_def: &serde_json::Value, new_def: &serde_json::Value) -> HashMap<String, String> {
    let old_fields = old_def["fields"].as_array().cloned().unwrap_or_default();
    let new_fields = new_def["fields"].as_array().cloned().unwrap_or_default();

    let mut rename_map: HashMap<String, String> = HashMap::new();
    let mut used_old = vec![false; old_fields.len()];
    let mut used_new = vec![false; new_fields.len()];

    // Names that survive are the same field regardless of display order or
    // changed validation. Matching positions first rotated data on reorder.
    for (old_index, old) in old_fields.iter().enumerate() {
        if let Some(new_index) = new_fields.iter().position(|new| new["name"] == old["name"]) {
            used_old[old_index] = true;
            used_new[new_index] = true;
        }
    }

    for i in 0..old_fields.len().min(new_fields.len()) {
        if used_old[i] || used_new[i] {
            continue;
        }
        let of = &old_fields[i];
        let nf = &new_fields[i];
        if of["name"] != nf["name"]
            && of["type"] == nf["type"]
            && of.get("required") == nf.get("required")
            && of.get("options") == nf.get("options")
            && let (Some(on), Some(nn)) = (of["name"].as_str(), nf["name"].as_str())
        {
            rename_map.insert(on.to_string(), nn.to_string());
            used_old[i] = true;
            used_new[i] = true;
        }
    }

    for (i, of) in old_fields.iter().enumerate() {
        if used_old[i] {
            continue;
        }
        for (j, nf) in new_fields.iter().enumerate() {
            if used_new[j] {
                continue;
            }
            if of["name"] != nf["name"]
                && of["type"] == nf["type"]
                && of.get("required") == nf.get("required")
                && of.get("options") == nf.get("options")
            {
                if let (Some(on), Some(nn)) = (of["name"].as_str(), nf["name"].as_str()) {
                    rename_map.insert(on.to_string(), nn.to_string());
                    used_old[i] = true;
                    used_new[j] = true;
                }
                break;
            }
        }
    }

    rename_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::traits::EntryRepository;
    use crate::test_helpers::{InMemoryCollectionRepository, InMemoryEntryRepository};
    use std::sync::Arc;

    fn test_repo() -> Arc<InMemoryCollectionRepository> {
        Arc::new(InMemoryCollectionRepository::new())
    }

    fn test_entry_repo() -> Arc<InMemoryEntryRepository> {
        Arc::new(InMemoryEntryRepository::new())
    }

    fn make_service() -> CollectionService {
        CollectionService::new(test_repo())
    }

    fn create_test_collection() -> Collection {
        Collection {
            id: "col-123".to_string(),
            site_id: "site-123".to_string(),
            name: "Test Collection".to_string(),
            slug: "test-collection".to_string(),
            definition: r#"{"fields": [{"name": "title", "type": "text"}]}"#.to_string(),
            is_singleton: false,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    #[tokio::test]
    async fn test_list_collections() {
        let service = make_service();

        let result = service.list_collections("site-123").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_singletons_only() {
        let service = make_service();

        let result = service.list_singletons_only("site-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_collection_not_found() {
        let service = make_service();

        let result = service.get_collection("site-123", "nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let service = make_service();

        let result = service.get_by_id("nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_create_collection_success() {
        let service = make_service();

        let result = service
            .create_collection("site-123", "My Collection", "my-collection", "{}", false)
            .await;
        assert!(result.is_ok());
        let col = result.unwrap();
        assert_eq!(col.name, "My Collection");
        assert_eq!(col.slug, "my-collection");
    }

    #[tokio::test]
    async fn test_create_singleton_collection() {
        let service = make_service();

        let result = service
            .create_collection("site-123", "My Singleton", "my-singleton", "{}", true)
            .await;
        assert!(result.is_ok());
        let col = result.unwrap();
        assert!(col.is_singleton);
    }

    #[tokio::test]
    async fn test_update_collection_success() {
        let repo = test_repo();
        repo.add_collection(create_test_collection());
        let service = CollectionService::new(repo);

        let result = service
            .update_collection("site-123", "test-collection", Some("Updated"), None, None)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "Updated");
    }

    #[tokio::test]
    async fn test_update_collection_not_found() {
        let service = make_service();

        let result = service
            .update_collection("site-123", "nonexistent", Some("Updated"), None, None)
            .await;
        assert!(matches!(result, Err(CollectionError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_collection_with_slug_change() {
        let repo = test_repo();
        repo.add_collection(create_test_collection());
        let service = CollectionService::new(repo);

        let result = service
            .update_collection("site-123", "test-collection", None, Some("new-slug"), None)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().slug, "new-slug");
    }

    #[tokio::test]
    async fn test_update_collection_with_definition_change() {
        let repo = test_repo();
        repo.add_collection(create_test_collection());
        let service = CollectionService::new(repo);

        let new_def = r#"{"fields": [{"name": "new_title", "type": "text"}]}"#;
        let result = service
            .update_collection("site-123", "test-collection", None, None, Some(new_def))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_collection_success() {
        let repo = test_repo();
        repo.add_collection(create_test_collection());
        let service = CollectionService::new(repo);

        let result = service.delete_collection("site-123", "test-collection").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_delete_collection_not_found() {
        let service = make_service();

        let result = service.delete_collection("site-123", "nonexistent").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_update_singleton_data_success() {
        let repo = test_repo();
        let entry_repo = test_entry_repo();
        let mut collection = create_test_collection();
        collection.is_singleton = true;
        repo.add_collection(collection.clone());
        let service = crate::services::singleton::SingletonService::new(
            repo,
            entry_repo.clone(),
            Arc::new(crate::test_helpers::InMemoryFileRepository::new()),
        );
        let response = service
            .update_singleton(
                "site-123",
                "test-collection",
                &serde_json::json!({"title":"Hello"}),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(response.data.unwrap()["title"], "Hello");
        let result = entry_repo
            .upsert_singleton_entry(crate::repository::traits::UpsertSingletonParams {
                site_id: "site-123",
                collection_id: &collection.id,
                slug: "test-collection",
                data: r#"{"title":"Hello"}"#,
                created_by: None,
                change_summary: None,
                expected_definition: None,
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_singleton_data_creates_entry() {
        let entry_repo = test_entry_repo();
        let result = entry_repo
            .upsert_singleton_entry(crate::repository::traits::UpsertSingletonParams {
                site_id: "site-123",
                collection_id: "col-123",
                slug: "x",
                data: r#"{"title":"Hello"}"#,
                created_by: None,
                change_summary: None,
                expected_definition: None,
            })
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn field_reordering_and_validation_changes_never_rename_surviving_names() {
        let old = serde_json::json!({"fields":[{"name":"first","type":"text"},{"name":"second","type":"text"}]});
        let reordered = serde_json::json!({"fields":[{"name":"second","type":"text"},{"name":"first","type":"text","required":true}]});
        assert!(compute_field_rename_map(&old, &reordered).is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_empty() {
        let old_def = json!({"fields": []});
        let new_def = json!({"fields": []});

        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_single_field_rename() {
        let old_def = json!({
            "fields": [
                {"name": "title", "type": "text", "required": true}
            ]
        });
        let new_def = json!({
            "fields": [
                {"name": "new_title", "type": "text", "required": true}
            ]
        });

        let result = compute_field_rename_map(&old_def, &new_def);
        assert_eq!(result.get("title"), Some(&"new_title".to_string()));
    }

    #[test]
    fn test_compute_field_rename_map_type_change_not_renamed() {
        let old_def = json!({
            "fields": [
                {"name": "title", "type": "text"}
            ]
        });
        let new_def = json!({
            "fields": [
                {"name": "new_title", "type": "number"}
            ]
        });

        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_no_rename() {
        let old_def = json!({
            "fields": [
                {"name": "title", "type": "text"},
                {"name": "body", "type": "richtext"}
            ]
        });
        let new_def = json!({
            "fields": [
                {"name": "title", "type": "text"},
                {"name": "body", "type": "richtext"}
            ]
        });

        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_multiple_renames() {
        let old_def = json!({
            "fields": [
                {"name": "a", "type": "text", "required": false},
                {"name": "b", "type": "text", "required": false},
                {"name": "c", "type": "text", "required": false}
            ]
        });
        let new_def = json!({
            "fields": [
                {"name": "x", "type": "text", "required": false},
                {"name": "y", "type": "text", "required": false},
                {"name": "z", "type": "text", "required": false}
            ]
        });

        let result = compute_field_rename_map(&old_def, &new_def);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_compute_field_rename_map_options_change_not_renamed() {
        let old_def = json!({
            "fields": [
                {"name": "title", "type": "text", "options": ["a", "b"]}
            ]
        });
        let new_def = json!({
            "fields": [
                {"name": "new_title", "type": "text", "options": ["a", "b", "c"]}
            ]
        });

        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_missing_fields_key() {
        let old_def = json!({"other": "data"});
        let new_def = json!({"fields": [{"name": "title", "type": "text"}]});

        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_partial_match() {
        let old_def = json!({
            "fields": [
                {"name": "title", "type": "text"},
                {"name": "body", "type": "text"}
            ]
        });
        let new_def = json!({
            "fields": [
                {"name": "title", "type": "text"},
                {"name": "new_body", "type": "text"}
            ]
        });

        let result = compute_field_rename_map(&old_def, &new_def);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("body"), Some(&"new_body".to_string()));
    }

    #[test]
    fn test_collection_error_into_response() {
        assert_eq!(
            CollectionError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            CollectionError::AlreadyExists.into_response().status(),
            axum::http::StatusCode::CONFLICT
        );
        assert_eq!(
            CollectionError::DatabaseError("bad".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
