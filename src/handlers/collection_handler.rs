use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use crate::middleware::auth::{AuthContext, check_read_access_repo, check_write_access_repo};
use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::repository::Repository;

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/collections",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 200, description = "List of collections", body = Vec<Collection>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, auth))]
pub async fn list_collections(
    auth: AuthContext,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_read_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match repository.collection.list(&site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/collections/{collection_slug}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("collection_slug" = String, Path, description = "Collection slug"),
    ),
    responses(
        (status = 200, description = "Collection details", body = Collection),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Collection not found"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, auth))]
pub async fn get_collection(
    auth: AuthContext,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_read_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match repository.collection.get_by_slug(&site_id, &collection_slug).await {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Collection not found"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/collections",
    params(("site_id" = String, Path, description = "Site ID")),
    request_body = CreateCollection,
    responses(
        (status = 201, description = "Collection created", body = Collection),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 409, description = "Collection name or slug already exists"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, auth, payload))]
pub async fn create_collection(
    auth: AuthContext,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<CreateCollection>,
) -> Response {
    if let Err((status, err)) = check_write_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    let definition_str = payload.definition.to_string();
    let id = Uuid::now_v7().to_string();
    let is_singleton = payload.is_singleton.unwrap_or(false);

    match repository.collection.create(
        &id,
        &site_id,
        &payload.name,
        &payload.slug,
        &definition_str,
        is_singleton,
    )
    .await
    {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(crate::repository::error::RepositoryError::UniqueViolation(_)) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Collection with this name or slug already exists"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}/collections/{collection_slug}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("collection_slug" = String, Path, description = "Collection slug"),
    ),
    request_body = UpdateCollection,
    responses(
        (status = 200, description = "Collection updated", body = Collection),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, auth, payload))]
pub async fn update_collection(
    auth: AuthContext,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateCollection>,
) -> Response {
    if let Err((status, err)) = check_write_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    let existing = match repository.collection.get_by_slug(&site_id, &collection_slug).await {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Collection not found"})),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let name = payload.name.unwrap_or_else(|| existing.name.clone());
    let new_slug = payload.slug.unwrap_or_else(|| existing.slug.clone());
    let definition_str = payload
        .definition
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| existing.definition.clone());

    if let Some(ref new_def_value) = payload.definition {
        let old_def: Option<serde_json::Value> = serde_json::from_str(&existing.definition).ok();
        let new_def: Option<serde_json::Value> = serde_json::from_value(new_def_value.clone()).ok();

        if let (Some(old_d), Some(new_d)) = (old_def, new_def) {
            let rename_map = compute_field_rename_map(&old_d, &new_d);

            if !rename_map.is_empty() {
                if existing.is_singleton {
                    let _ = repository.collection.migrate_singleton_field_renames(
                        &existing,
                        &rename_map,
                    ).await;
                } else if let Ok(items) = repository.collection.get_content_for_migration(&existing.id).await {
                    let _ = repository.collection.migrate_content_field_renames(&items, &rename_map).await;
                }
            }
        }
    }

    match repository.collection.update(&existing.id, &name, &new_slug, &definition_str).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/collections/{collection_slug}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("collection_slug" = String, Path, description = "Collection slug"),
    ),
    responses(
        (status = 204, description = "Collection deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, auth))]
pub async fn delete_collection(
    auth: AuthContext,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_write_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match repository.collection.delete(&site_id, &collection_slug).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

fn compute_field_rename_map(
    old_def: &serde_json::Value,
    new_def: &serde_json::Value,
) -> std::collections::HashMap<String, String> {
    let old_fields = old_def["fields"].as_array().cloned().unwrap_or_default();
    let new_fields = new_def["fields"].as_array().cloned().unwrap_or_default();

    let mut rename_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_field_rename_map_no_changes() {
        let old_def = serde_json::json!({
            "fields": [
                {"name": "title", "type": "text", "required": true}
            ]
        });
        let new_def = serde_json::json!({
            "fields": [
                {"name": "title", "type": "text", "required": true}
            ]
        });
        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_simple_rename() {
        let old_def = serde_json::json!({
            "fields": [
                {"name": "old_name", "type": "text", "required": true}
            ]
        });
        let new_def = serde_json::json!({
            "fields": [
                {"name": "new_name", "type": "text", "required": true}
            ]
        });
        let result = compute_field_rename_map(&old_def, &new_def);
        assert_eq!(result.get("old_name"), Some(&"new_name".to_string()));
    }

    #[test]
    fn test_compute_field_rename_map_type_mismatch_no_rename() {
        let old_def = serde_json::json!({
            "fields": [
                {"name": "field", "type": "text", "required": true}
            ]
        });
        let new_def = serde_json::json!({
            "fields": [
                {"name": "renamed", "type": "number", "required": true}
            ]
        });
        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_required_mismatch_no_rename() {
        let old_def = serde_json::json!({
            "fields": [
                {"name": "field", "type": "text", "required": true}
            ]
        });
        let new_def = serde_json::json!({
            "fields": [
                {"name": "renamed", "type": "text", "required": false}
            ]
        });
        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_options_mismatch_no_rename() {
        let old_def = serde_json::json!({
            "fields": [
                {"name": "field", "type": "select", "required": true, "options": ["a", "b"]}
            ]
        });
        let new_def = serde_json::json!({
            "fields": [
                {"name": "renamed", "type": "select", "required": true, "options": ["a", "b", "c"]}
            ]
        });
        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_multiple_fields() {
        let old_def = serde_json::json!({
            "fields": [
                {"name": "title", "type": "text", "required": true},
                {"name": "body", "type": "richtext", "required": false}
            ]
        });
        let new_def = serde_json::json!({
            "fields": [
                {"name": "heading", "type": "text", "required": true},
                {"name": "content", "type": "richtext", "required": false}
            ]
        });
        let result = compute_field_rename_map(&old_def, &new_def);
        assert_eq!(result.get("title"), Some(&"heading".to_string()));
        assert_eq!(result.get("body"), Some(&"content".to_string()));
    }

    #[test]
    fn test_compute_field_rename_map_unordered_matching() {
        let old_def = serde_json::json!({
            "fields": [
                {"name": "first", "type": "text", "required": true},
                {"name": "second", "type": "text", "required": true}
            ]
        });
        let new_def = serde_json::json!({
            "fields": [
                {"name": "first_renamed", "type": "text", "required": true},
                {"name": "second_renamed", "type": "text", "required": true}
            ]
        });
        let result = compute_field_rename_map(&old_def, &new_def);
        assert_eq!(result.get("first"), Some(&"first_renamed".to_string()));
        assert_eq!(result.get("second"), Some(&"second_renamed".to_string()));
    }

    #[test]
    fn test_compute_field_rename_map_empty_fields() {
        let old_def = serde_json::json!({"fields": []});
        let new_def = serde_json::json!({"fields": []});
        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_field_rename_map_missing_fields_key() {
        let old_def = serde_json::json!({});
        let new_def = serde_json::json!({"fields": [{"name": "test", "type": "text"}]});
        let result = compute_field_rename_map(&old_def, &new_def);
        assert!(result.is_empty());
    }
}
