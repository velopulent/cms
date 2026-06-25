use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::Value;
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::models::entry::{Entry, EntryRevision};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{
    CollectionRepository, EntriesListResult, EntryRepository, FileRepository, ListEntriesParams, RevisionsListResult,
    UpdateEntryParams,
};
use crate::services::search::queue::{OP_DELETE, OP_INDEX, SearchQueue};
use crate::services::search::{SearchError, SearchParams, SearchService};
use crate::storage::StorageProvider;

#[derive(Clone)]
pub struct EntryService {
    entry_repo: Arc<dyn EntryRepository>,
    file_repo: Arc<dyn FileRepository>,
    collection_repo: Arc<dyn CollectionRepository>,
    /// Read side: ranked queries against the index. `None` falls back to SQL `LIKE`.
    search: Option<Arc<SearchService>>,
    /// Write side: enqueue index updates for the server's indexer to apply.
    search_queue: Option<Arc<SearchQueue>>,
}

/// Fields for [`EntryService::update_entry`]. All but `id`/`site_id` are optional;
/// `None` leaves the existing value unchanged.
pub struct UpdateEntryInput<'a> {
    pub id: &'a str,
    pub site_id: &'a str,
    pub data: Option<&'a Value>,
    pub slug: Option<&'a str>,
    pub status: Option<&'a str>,
    pub created_by: Option<&'a str>,
    pub change_summary: Option<&'a str>,
}

#[derive(Error, Debug)]
pub enum EntryError {
    #[error("Not found")]
    NotFound,

    #[error("Revision not found")]
    RevisionNotFound,

    #[error("Entry with this slug already exists for this collection")]
    AlreadyExists,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl EntryError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            EntryError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))),
            EntryError::RevisionNotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Revision not found"}))),
            EntryError::AlreadyExists => (
                StatusCode::CONFLICT,
                Json(json!({"error": "Entry with this slug already exists for this collection"})),
            ),
            EntryError::ValidationFailed(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            EntryError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))),
        };
        (status, body).into_response()
    }
}

impl EntryService {
    pub fn new(
        entry_repo: Arc<dyn EntryRepository>,
        file_repo: Arc<dyn FileRepository>,
        collection_repo: Arc<dyn CollectionRepository>,
    ) -> Self {
        Self {
            entry_repo,
            file_repo,
            collection_repo,
            search: None,
            search_queue: None,
        }
    }

    /// Attach the read-side search engine for ranked querying. Builder form keeps
    /// the `new` signature stable for tests.
    pub fn with_search(mut self, search: Option<Arc<SearchService>>) -> Self {
        self.search = search;
        self
    }

    /// Attach the write-side index queue. Content writes enqueue here for the
    /// server's indexer to apply.
    pub fn with_queue(mut self, queue: Option<Arc<SearchQueue>>) -> Self {
        self.search_queue = queue;
        self
    }

    /// Best-effort enqueue of an entry upsert. Enqueue failures are logged but never
    /// fail the originating write — the index is derived and rebuildable.
    async fn enqueue_upsert(&self, entry: &Entry) {
        if let Some(queue) = &self.search_queue
            && let Err(e) = queue.enqueue(&entry.id, &entry.site_id, OP_INDEX).await
        {
            warn!("Failed to enqueue entry {} for search indexing: {}", entry.id, e);
        }
    }

    /// Best-effort enqueue of an entry deletion.
    async fn enqueue_delete(&self, id: &str, site_id: &str) {
        if let Some(queue) = &self.search_queue
            && let Err(e) = queue.enqueue(id, site_id, OP_DELETE).await
        {
            warn!("Failed to enqueue entry {} deletion for search indexing: {}", id, e);
        }
    }

    /// Validate `relation` fields against the database: every referenced id must
    /// exist as an entry in the relation's target collection (resolved by slug
    /// within the same site). Scalar/array shape, length, and `max_select` are
    /// already checked by the pure [`validate_entry_data`].
    async fn validate_relations(&self, site_id: &str, fields: &[Value], data: &Value) -> Result<(), EntryError> {
        let obj = match data.as_object() {
            Some(o) => o,
            None => return Ok(()),
        };

        for field_def in fields {
            if field_def.get("type").and_then(|t| t.as_str()) != Some("relation") {
                continue;
            }
            let name = field_def.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let target_slug = match field_def.get("target_collection").and_then(|t| t.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let value = match obj.get(name) {
                Some(v) if !v.is_null() => v,
                _ => continue,
            };

            let target = self
                .collection_repo
                .get_by_slug(site_id, target_slug)
                .await
                .map_err(|e| EntryError::DatabaseError(e.to_string()))?
                .ok_or_else(|| {
                    EntryError::ValidationFailed(format!(
                        "Relation field '{}' targets unknown collection '{}'",
                        name, target_slug
                    ))
                })?;

            let ids: Vec<&str> = match value {
                Value::String(s) => vec![s.as_str()],
                Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
                _ => {
                    return Err(EntryError::ValidationFailed(format!(
                        "Relation field '{}' must be an entry id or array of ids",
                        name
                    )));
                }
            };

            for id in ids {
                let exists = self
                    .entry_repo
                    .get_by_id(id, site_id, false)
                    .await
                    .map_err(|e| EntryError::DatabaseError(e.to_string()))?
                    .map(|e| e.collection_id == target.id)
                    .unwrap_or(false);
                if !exists {
                    return Err(EntryError::ValidationFailed(format!(
                        "Relation field '{}' references non-existent entry '{}' in '{}'",
                        name, id, target_slug
                    )));
                }
            }
        }

        Ok(())
    }

    /// After an entry is deleted, delete any entries that reference it through a
    /// relation field marked `cascade_delete`. Best-effort and recursive (chains
    /// of cascades). Failures are logged, not propagated — the primary delete has
    /// already succeeded.
    async fn cascade_relation_deletes(&self, site_id: &str, deleted_collection_id: &str, deleted_id: &str) {
        let target_slug = match self.collection_repo.get_by_id(deleted_collection_id).await {
            Ok(Some(c)) => c.slug,
            _ => return,
        };
        let collections = match self.collection_repo.list(site_id).await {
            Ok(cs) => cs,
            Err(e) => {
                warn!("cascade delete: failed to list collections: {}", e);
                return;
            }
        };

        for col in collections {
            let definition: Value = match serde_json::from_str(&col.definition) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let Some(fields) = definition.get("fields").and_then(|f| f.as_array()) else {
                continue;
            };
            let cascade_fields: Vec<String> = fields
                .iter()
                .filter(|f| {
                    f.get("type").and_then(|t| t.as_str()) == Some("relation")
                        && f.get("target_collection").and_then(|t| t.as_str()) == Some(target_slug.as_str())
                        && f.get("cascade_delete").and_then(|c| c.as_bool()).unwrap_or(false)
                })
                .filter_map(|f| f.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect();
            if cascade_fields.is_empty() {
                continue;
            }

            let entries = match self.entry_repo.get_by_collection_id(&col.id, None, false).await {
                Ok(e) => e,
                Err(_) => continue,
            };
            for e in entries {
                let data: Value = match serde_json::from_str(&e.data) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let references = cascade_fields.iter().any(|fname| match data.get(fname) {
                    Some(Value::String(s)) => s == deleted_id,
                    Some(Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(deleted_id)),
                    _ => false,
                });
                if references {
                    info!(
                        "cascade delete: removing entry {} referencing deleted {}",
                        e.id, deleted_id
                    );
                    let _ = Box::pin(self.delete_entry(&e.id, site_id)).await;
                }
            }
        }
    }

    pub async fn list_entries(&self, params: ListEntriesParams<'_>) -> Result<EntriesListResult, EntryError> {
        // When a search index is available and a query is provided, use ranked
        // full-text search; otherwise fall back to the repository's SQL filter.
        if let (Some(search), Some(query)) = (&self.search, params.search)
            && !query.trim().is_empty()
        {
            match self.search_via_index(search, &params, query).await {
                Ok(result) => return Ok(result),
                Err(e) => warn!("Search index query failed; falling back to SQL: {}", e),
            }
        }

        self.entry_repo
            .list(params)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))
    }

    /// Resolve a ranked search against the index, then hydrate full rows from the
    /// database in rank order. `collection_slug` is resolved to an id first since
    /// the index filters by collection id.
    async fn search_via_index(
        &self,
        search: &SearchService,
        params: &ListEntriesParams<'_>,
        query: &str,
    ) -> Result<EntriesListResult, SearchError> {
        let mut collection_id = params.collection_id.map(str::to_string);
        if collection_id.is_none()
            && let Some(slug) = params.collection_slug
        {
            match self.collection_repo.get_by_slug(params.site_id, slug).await {
                Ok(Some(c)) => collection_id = Some(c.id),
                // Unknown collection slug → no possible matches.
                Ok(None) => {
                    return Ok(EntriesListResult {
                        items: Vec::new(),
                        total: 0,
                        page: params.page,
                        per_page: params.per_page,
                    });
                }
                Err(e) => return Err(SearchError::Repository(e.to_string())),
            }
        }

        let hits = search.search_entries(&SearchParams {
            site_id: params.site_id,
            collection_id: collection_id.as_deref(),
            status: params.status,
            published_only: params.published_only,
            query,
            page: params.page,
            per_page: params.per_page,
        })?;

        let mut items = Vec::with_capacity(hits.ids.len());
        for id in &hits.ids {
            // Re-check site + publish scope at the DB; the index may briefly lag.
            if let Ok(Some(entry)) = self
                .entry_repo
                .get_by_id(id, params.site_id, params.published_only)
                .await
            {
                items.push(entry);
            }
        }

        Ok(EntriesListResult {
            items,
            total: hits.total as i64,
            page: params.page,
            per_page: params.per_page,
        })
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
        created_by: Option<&str>,
    ) -> Result<Entry, EntryError> {
        let id = Uuid::now_v7().to_string();

        debug!(
            "Creating entry: site_id={}, collection_id={}, slug={}, has_creator={}",
            site_id,
            collection_id,
            slug,
            created_by.is_some()
        );

        let collection = self
            .collection_repo
            .get_by_id(collection_id)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))?;

        if let Some(ref c) = collection
            && c.is_singleton
        {
            warn!(
                "Attempted to create entry for singleton collection: id={}, slug={}",
                collection_id, c.slug
            );
            return Err(EntryError::ValidationFailed(
                "Cannot create entries for singleton collections via /entries; use /singletons/{slug}".into(),
            ));
        }

        if let Some(ref c) = collection
            && let Ok(definition) = serde_json::from_str::<Value>(&c.definition)
            && let Some(fields) = definition.get("fields").and_then(|f| f.as_array())
        {
            if let Some(err) = super::definition_validation::validate_entry_data(data, fields) {
                return Err(EntryError::ValidationFailed(err));
            }
            self.validate_relations(site_id, fields, data).await?;
        }

        let data_str = data.to_string();

        self.entry_repo
            .create(&id, site_id, collection_id, &data_str, slug, created_by)
            .await
            .map_err(|e| {
                error!(
                    "Failed to create entry in repository: site_id={}, collection_id={}, slug={}, error={}",
                    site_id, collection_id, slug, e
                );
                match e {
                    RepositoryError::UniqueViolation(_) => EntryError::AlreadyExists,
                    _ => EntryError::DatabaseError(e.to_string()),
                }
            })?;

        debug!("Entry created in repository: id={}", id);

        // Sync file references
        if let Err(e) = self.entry_repo.sync_file_references(&id, site_id, data).await {
            warn!("Failed to sync file references for entry {}: {}", id, e);
            // Continue anyway as this is not critical
        }

        match self.entry_repo.get_by_id(&id, site_id, false).await {
            Ok(Some(entry)) => {
                info!(
                    "Entry created successfully: id={}, site_id={}, slug={}",
                    id, site_id, slug
                );
                self.enqueue_upsert(&entry).await;
                Ok(entry)
            }
            Ok(None) => {
                error!("Entry not found after creation: id={}", id);
                Err(EntryError::NotFound)
            }
            Err(e) => {
                error!("Failed to fetch entry after creation: id={}, error={}", id, e);
                Err(EntryError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn update_entry(&self, input: UpdateEntryInput<'_>) -> Result<Entry, EntryError> {
        let UpdateEntryInput {
            id,
            site_id,
            data,
            slug,
            status,
            created_by,
            change_summary,
        } = input;
        debug!("Updating entry: id={}, site_id={}", id, site_id);

        let existing = self
            .entry_repo
            .get_by_id(id, site_id, false)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch existing entry for update: id={}, site_id={}, error={}",
                    id, site_id, e
                );
                EntryError::DatabaseError(e.to_string())
            })?
            .ok_or(EntryError::NotFound)?;

        debug!("Fetched existing entry: id={}, site_id={}", id, site_id);

        let resolved_data = match data {
            Some(d) => d.clone(),
            None => serde_json::from_str(&existing.data).unwrap_or(Value::Null),
        };

        // Validate entry data against collection definition
        if let Ok(Some(collection)) = self.collection_repo.get_by_id(&existing.collection_id).await
            && let Ok(definition) = serde_json::from_str::<Value>(&collection.definition)
            && let Some(fields) = definition.get("fields").and_then(|f| f.as_array())
        {
            if let Some(err) = super::definition_validation::validate_entry_data(&resolved_data, fields) {
                return Err(EntryError::ValidationFailed(err));
            }
            self.validate_relations(site_id, fields, &resolved_data).await?;
        }

        let data_str = resolved_data.to_string();
        let final_slug = slug.unwrap_or(&existing.slug);
        let final_status = status.unwrap_or(&existing.status);

        debug!(
            "Updating entry fields: data_changed={}, slug_changed={}, status_changed={}",
            data.is_some(),
            slug.is_some(),
            status.is_some()
        );

        self.entry_repo
            .update(UpdateEntryParams {
                id,
                site_id,
                data: &data_str,
                slug: final_slug,
                status: final_status,
                created_by,
                change_summary,
            })
            .await
            .map_err(|e| {
                error!(
                    "Failed to update entry in repository: id={}, site_id={}, error={}",
                    id, site_id, e
                );
                match e {
                    RepositoryError::UniqueViolation(_) => EntryError::AlreadyExists,
                    _ => EntryError::DatabaseError(e.to_string()),
                }
            })?;

        debug!("Entry updated in repository: id={}", id);

        // Sync file references
        if let Err(e) = self.entry_repo.sync_file_references(id, site_id, &resolved_data).await {
            warn!("Failed to sync file references for entry {}: {}", id, e);
            // Continue anyway as this is not critical
        }

        match self.entry_repo.get_by_id(id, site_id, false).await {
            Ok(Some(entry)) => {
                info!("Entry updated successfully: id={}, site_id={}", id, site_id);
                self.enqueue_upsert(&entry).await;
                Ok(entry)
            }
            Ok(None) => {
                error!("Entry not found after update: id={}", id);
                Err(EntryError::NotFound)
            }
            Err(e) => {
                error!("Failed to fetch entry after update: id={}, error={}", id, e);
                Err(EntryError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn delete_entry(&self, id: &str, site_id: &str) -> Result<u64, EntryError> {
        info!("Deleting entry: id={}, site_id={}", id, site_id);

        // Resolve the entry first so we can cascade relation deletes after removal.
        let collection_id = self
            .entry_repo
            .get_by_id(id, site_id, false)
            .await
            .ok()
            .flatten()
            .map(|e| e.collection_id);

        match self.entry_repo.delete(id, site_id).await {
            Ok(deleted_count) => {
                info!("Entry deleted successfully: id={}, deleted_count={}", id, deleted_count);
                self.enqueue_delete(id, site_id).await;
                if deleted_count > 0
                    && let Some(collection_id) = collection_id
                {
                    self.cascade_relation_deletes(site_id, &collection_id, id).await;
                }
                Ok(deleted_count)
            }
            Err(e) => {
                error!("Failed to delete entry: id={}, site_id={}, error={}", id, site_id, e);
                Err(EntryError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn publish_entry(&self, id: &str, site_id: &str) -> Result<Entry, EntryError> {
        info!("Publishing entry: id={}, site_id={}", id, site_id);

        let entry = self.entry_repo.publish(id, site_id).await.map_err(|e| {
            error!("Failed to publish entry: error={}", e);
            match e {
                RepositoryError::NotFound => EntryError::NotFound,
                _ => EntryError::DatabaseError(e.to_string()),
            }
        })?;
        self.enqueue_upsert(&entry).await;
        Ok(entry)
    }

    pub async fn unpublish_entry(&self, id: &str, site_id: &str) -> Result<Entry, EntryError> {
        info!("Unpublishing entry: id={}, site_id={}", id, site_id);

        let entry = self.entry_repo.unpublish(id, site_id).await.map_err(|e| {
            error!("Failed to unpublish entry: error={}", e);
            match e {
                RepositoryError::NotFound => EntryError::NotFound,
                _ => EntryError::DatabaseError(e.to_string()),
            }
        })?;
        self.enqueue_upsert(&entry).await;
        Ok(entry)
    }

    pub async fn list_revisions(
        &self,
        entry_id: &str,
        site_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<RevisionsListResult, EntryError> {
        self.entry_repo
            .get_by_id(entry_id, site_id, false)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))?
            .ok_or(EntryError::NotFound)?;

        self.entry_repo
            .list_revisions(entry_id, page, per_page)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))
    }

    pub async fn get_revision(
        &self,
        entry_id: &str,
        site_id: &str,
        revision_number: i64,
    ) -> Result<Option<EntryRevision>, EntryError> {
        self.entry_repo
            .get_by_id(entry_id, site_id, false)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))?
            .ok_or(EntryError::NotFound)?;

        self.entry_repo
            .get_revision(entry_id, revision_number)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))
    }

    pub async fn restore_revision(
        &self,
        entry_id: &str,
        site_id: &str,
        revision_number: i64,
        created_by: Option<&str>,
    ) -> Result<Entry, EntryError> {
        self.entry_repo
            .get_by_id(entry_id, site_id, false)
            .await
            .map_err(|e| EntryError::DatabaseError(e.to_string()))?
            .ok_or(EntryError::NotFound)?;

        let entry = self
            .entry_repo
            .restore_revision(entry_id, revision_number, created_by)
            .await
            .map_err(|e| match e {
                RepositoryError::NotFound => EntryError::RevisionNotFound,
                _ => EntryError::DatabaseError(e.to_string()),
            })?;
        self.enqueue_upsert(&entry).await;
        Ok(entry)
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

        if !file_ids.is_empty()
            && let Ok(file_items) = self.file_repo.get_by_ids(site_id, &file_ids).await
        {
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
    use crate::test_helpers::{InMemoryEntryRepository, InMemoryFileRepository};
    use std::sync::Arc;

    use crate::test_helpers::InMemoryCollectionRepository;

    fn test_entry_repo() -> Arc<InMemoryEntryRepository> {
        Arc::new(InMemoryEntryRepository::new())
    }

    fn test_file_repo() -> Arc<InMemoryFileRepository> {
        Arc::new(InMemoryFileRepository::new())
    }

    fn test_collection_repo() -> Arc<InMemoryCollectionRepository> {
        Arc::new(InMemoryCollectionRepository::new())
    }

    fn create_test_entry() -> Entry {
        Entry {
            id: "entry-123".to_string(),
            site_id: "site-123".to_string(),
            collection_id: "col-123".to_string(),
            data: r#"{"title": "Test Entry"}"#.to_string(),
            slug: "test-entry".to_string(),
            status: "draft".to_string(),
            singleton_collection_id: None,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
            published_at: None,
        }
    }

    #[tokio::test]
    async fn test_list_entries() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

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
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service.get_entry("nonexistent", "site-123", false).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_create_entry_success() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({"title": "New Entry"});
        let result = service
            .create_entry("site-123", "col-123", &data, "new-entry", None)
            .await;
        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.slug, "new-entry");
        assert_eq!(entry.status, "draft");
    }

    #[tokio::test]
    async fn test_create_entry_empty_data() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({});
        let result = service
            .create_entry("site-123", "col-123", &data, "empty-entry", None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_entry_success() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let new_data = json!({"title": "Updated Title"});
        let result = service
            .update_entry(UpdateEntryInput {
                id: "entry-123",
                site_id: "site-123",
                data: Some(&new_data),
                slug: Some("updated-slug"),
                status: None,
                created_by: None,
                change_summary: None,
            })
            .await;
        assert!(result.is_ok());
        let entry = result.unwrap();
        assert_eq!(entry.slug, "updated-slug");
    }

    #[tokio::test]
    async fn test_update_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service
            .update_entry(UpdateEntryInput {
                id: "nonexistent",
                site_id: "site-123",
                data: Some(&json!({})),
                slug: None,
                status: None,
                created_by: None,
                change_summary: None,
            })
            .await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_entry_status_only() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service
            .update_entry(UpdateEntryInput {
                id: "entry-123",
                site_id: "site-123",
                data: None,
                slug: None,
                status: Some("published"),
                created_by: None,
                change_summary: None,
            })
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, "published");
    }

    #[tokio::test]
    async fn test_delete_entry_success() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service.delete_entry("entry-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_delete_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service.delete_entry("nonexistent", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_publish_entry_success() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service.publish_entry("entry-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, "published");
    }

    #[tokio::test]
    async fn test_publish_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

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
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service.unpublish_entry("entry-123", "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, "draft");
    }

    #[tokio::test]
    async fn test_unpublish_entry_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let result = service.unpublish_entry("nonexistent", "site-123").await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_resolve_entry_files_with_no_files() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let entry = create_test_entry();
        let storage = Arc::new(MockStorage::default());
        let result = service.resolve_entry_files(&entry, storage).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_entries_list_files() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let entries = vec![create_test_entry()];
        let result = service.resolve_entries_list_files(&entries).await;
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_extract_file_ids_from_value() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

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
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

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
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({"title": "No files here"});
        let ids = service.extract_file_ids_from_value(&data);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_extract_file_ids_invalid_format() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

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
            EntryError::RevisionNotFound.into_response().status(),
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

    #[tokio::test]
    async fn test_list_revisions() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({"title": "Test"});
        let entry = service
            .create_entry("site-123", "col-123", &data, "test-entry", None)
            .await
            .unwrap();

        let result = service.list_revisions(&entry.id, "site-123", 1, 10).await;
        assert!(result.is_ok());
        let revisions = result.unwrap();
        assert_eq!(revisions.total, 1);
    }

    #[tokio::test]
    async fn test_list_revisions_entry_not_found() {
        let service = EntryService::new(test_entry_repo(), test_file_repo(), test_collection_repo());
        let result = service.list_revisions("nonexistent", "site-123", 1, 10).await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_revision() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({"title": "Test"});
        let entry = service
            .create_entry("site-123", "col-123", &data, "test-entry", None)
            .await
            .unwrap();

        let result = service.get_revision(&entry.id, "site-123", 1).await;
        assert!(result.is_ok());
        let rev = result.unwrap();
        assert!(rev.is_some());
        assert_eq!(rev.unwrap().revision_number, 1);
    }

    #[tokio::test]
    async fn test_get_revision_entry_not_found() {
        let service = EntryService::new(test_entry_repo(), test_file_repo(), test_collection_repo());
        let result = service.get_revision("nonexistent", "site-123", 1).await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_revision_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({"title": "Test"});
        let entry = service
            .create_entry("site-123", "col-123", &data, "test-entry", None)
            .await
            .unwrap();

        let result = service.get_revision(&entry.id, "site-123", 999).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_restore_revision() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({"title": "Original"});
        let entry = service
            .create_entry("site-123", "col-123", &data, "test-entry", None)
            .await
            .unwrap();

        let updated = json!({"title": "Updated"});
        service
            .update_entry(UpdateEntryInput {
                id: &entry.id,
                site_id: "site-123",
                data: Some(&updated),
                slug: None,
                status: None,
                created_by: None,
                change_summary: None,
            })
            .await
            .unwrap();

        let result = service.restore_revision(&entry.id, "site-123", 1, None).await;
        assert!(result.is_ok());
        let restored = result.unwrap();
        assert_eq!(restored.id, entry.id);
    }

    #[tokio::test]
    async fn test_restore_revision_entry_not_found() {
        let service = EntryService::new(test_entry_repo(), test_file_repo(), test_collection_repo());
        let result = service.restore_revision("nonexistent", "site-123", 1, None).await;
        assert!(matches!(result, Err(EntryError::NotFound)));
    }

    #[tokio::test]
    async fn test_restore_revision_not_found() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let service = EntryService::new(entry_repo, file_repo, test_collection_repo());

        let data = json!({"title": "Test"});
        let entry = service
            .create_entry("site-123", "col-123", &data, "test-entry", None)
            .await
            .unwrap();

        let result = service.restore_revision(&entry.id, "site-123", 999, None).await;
        assert!(matches!(result, Err(EntryError::RevisionNotFound)));
    }

    #[tokio::test]
    async fn test_create_entry_validation_failed() {
        let entry_repo = test_entry_repo();
        let file_repo = test_file_repo();
        let col_repo = test_collection_repo();

        let collection = crate::models::collection::Collection {
            id: "col-123".to_string(),
            site_id: "site-123".to_string(),
            name: "Posts".to_string(),
            slug: "posts".to_string(),
            definition: r#"{"fields":[{"name":"title","type":"number","required":true}]}"#.to_string(),
            is_singleton: false,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        };
        col_repo.add_collection(collection);

        let service = EntryService::new(entry_repo, file_repo, col_repo);
        let data = json!({"title": "not-a-number"});
        let result = service
            .create_entry("site-123", "col-123", &data, "bad-entry", None)
            .await;
        assert!(matches!(result, Err(EntryError::ValidationFailed(_))));
    }

    #[tokio::test]
    async fn test_update_entry_validation_failed() {
        let entry_repo = test_entry_repo();
        entry_repo.add_entry(create_test_entry());
        let file_repo = test_file_repo();
        let col_repo = test_collection_repo();

        let collection = crate::models::collection::Collection {
            id: "col-123".to_string(),
            site_id: "site-123".to_string(),
            name: "Posts".to_string(),
            slug: "posts".to_string(),
            definition: r#"{"fields":[{"name":"title","type":"number","required":true}]}"#.to_string(),
            is_singleton: false,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        };
        col_repo.add_collection(collection);

        let service = EntryService::new(entry_repo, file_repo, col_repo);
        let data = json!({"title": "not-a-number"});
        let result = service
            .update_entry(UpdateEntryInput {
                id: "entry-123",
                site_id: "site-123",
                data: Some(&data),
                slug: None,
                status: None,
                created_by: None,
                change_summary: None,
            })
            .await;
        assert!(matches!(result, Err(EntryError::ValidationFailed(_))));
    }
}
