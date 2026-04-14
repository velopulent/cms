use std::collections::HashMap;
use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::models::collection::Collection;
use crate::repository::Repository;
use crate::repository::error::RepositoryError;

#[derive(Clone)]
pub struct CollectionService {
    repository: Arc<Repository>,
}

#[derive(Error, Debug)]
pub enum CollectionError {
    #[error("Not found")]
    NotFound,

    #[error("Collection with this name or slug already exists")]
    AlreadyExists,

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl CollectionError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            CollectionError::NotFound => {
                (StatusCode::NOT_FOUND, Json(json!({"error": "Collection not found"})))
            }
            CollectionError::AlreadyExists => {
                (StatusCode::CONFLICT, Json(json!({"error": "Collection with this name or slug already exists"})))
            }
            CollectionError::DatabaseError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
            }
        };
        (status, body).into_response()
    }
}

impl CollectionService {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }

    pub async fn list_collections(&self, site_id: &str) -> Result<Vec<Collection>, CollectionError> {
        self.repository
            .collection
            .list(site_id)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn list_singletons_only(&self, site_id: &str) -> Result<Vec<Collection>, CollectionError> {
        self.repository
            .collection
            .list_singletons_only(site_id)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn get_collection(&self, site_id: &str, slug: &str) -> Result<Option<Collection>, CollectionError> {
        self.repository
            .collection
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<Collection>, CollectionError> {
        self.repository
            .collection
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

        self.repository
            .collection
            .create(&id, site_id, name, slug, definition, is_singleton)
            .await
            .map_err(|e| match e {
                RepositoryError::UniqueViolation(_) => CollectionError::AlreadyExists,
                _ => CollectionError::DatabaseError(e.to_string()),
            })
    }

    pub async fn update_collection(
        &self,
        site_id: &str,
        slug: &str,
        name: Option<&str>,
        new_slug: Option<&str>,
        definition: Option<&str>,
    ) -> Result<Collection, CollectionError> {
        let existing = self
            .repository
            .collection
            .get_by_slug(site_id, slug)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))?
            .ok_or(CollectionError::NotFound)?;

        let name = name.unwrap_or(&existing.name);
        let new_slug = new_slug.unwrap_or(&existing.slug);
        let definition_str = definition
            .map(|s| s.to_string())
            .unwrap_or_else(|| existing.definition.clone());

        if let Some(ref new_def) = definition {
            let old_def: Option<serde_json::Value> = serde_json::from_str(&existing.definition).ok();
            let new_def_parsed: Option<serde_json::Value> = serde_json::from_str(new_def).ok();

            if let (Some(old_d), Some(new_d)) = (old_def, new_def_parsed) {
                let rename_map = compute_field_rename_map(&old_d, &new_d);

                if !rename_map.is_empty() {
                    if existing.is_singleton {
                        let _ = self
                            .repository
                            .collection
                            .migrate_singleton_field_renames(&existing, &rename_map)
                            .await;
                    } else if let Ok(items) = self.repository.collection.get_content_for_migration(&existing.id).await {
                        let _ = self
                            .repository
                            .collection
                            .migrate_content_field_renames(&items, &rename_map)
                            .await;
                    }
                }
            }
        }

        self.repository
            .collection
            .update(&existing.id, name, &new_slug, &definition_str)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn delete_collection(&self, site_id: &str, slug: &str) -> Result<u64, CollectionError> {
        self.repository
            .collection
            .delete(site_id, slug)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }

    pub async fn update_singleton_data(&self, id: &str, data: &str) -> Result<Collection, CollectionError> {
        self.repository
            .collection
            .update_singleton_data(id, data)
            .await
            .map_err(|e| CollectionError::DatabaseError(e.to_string()))
    }
}

pub fn compute_field_rename_map(
    old_def: &serde_json::Value,
    new_def: &serde_json::Value,
) -> HashMap<String, String> {
    let old_fields = old_def["fields"].as_array().cloned().unwrap_or_default();
    let new_fields = new_def["fields"].as_array().cloned().unwrap_or_default();

    let mut rename_map: HashMap<String, String> = HashMap::new();
    let mut used_old = vec![false; old_fields.len()];
    let mut used_new = vec![false; new_fields.len()];

    for i in 0..old_fields.len().min(new_fields.len()) {
        let of = &old_fields[i];
        let nf = &new_fields[i];
        if of["name"] != nf["name"]
            && of["type"] == nf["type"]
            && of.get("required") == nf.get("required")
            && of.get("options") == nf.get("options")
        {
            if let (Some(on), Some(nn)) = (of["name"].as_str(), nf["name"].as_str()) {
                rename_map.insert(on.to_string(), nn.to_string());
                used_old[i] = true;
                used_new[i] = true;
            }
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
