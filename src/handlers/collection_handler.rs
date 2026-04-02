use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::middleware::auth::{AuthContext, check_read_access, check_write_access};
use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::repository::collection as collection_repo;

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
pub async fn list_collections(
    auth: AuthContext,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_read_access(&auth, &pool, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match collection_repo::list(&pool, &site_id).await {
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
pub async fn get_collection(
    auth: AuthContext,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_read_access(&auth, &pool, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match collection_repo::get_by_slug(&pool, &site_id, &collection_slug).await {
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
pub async fn create_collection(
    auth: AuthContext,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateCollection>,
) -> Response {
    if let Err((status, err)) = check_write_access(&auth, &pool, &site_id).await {
        return (status, Json(err)).into_response();
    }

    let definition_str = payload.definition.to_string();
    let id = Uuid::now_v7().to_string();
    let is_singleton = payload.is_singleton.unwrap_or(false);

    match collection_repo::create(
        &pool,
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
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
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
pub async fn update_collection(
    auth: AuthContext,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateCollection>,
) -> Response {
    if let Err((status, err)) = check_write_access(&auth, &pool, &site_id).await {
        return (status, Json(err)).into_response();
    }

    let existing = match collection_repo::get_by_slug(&pool, &site_id, &collection_slug).await {
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
                    collection_repo::migrate_singleton_field_renames(
                        &pool,
                        &existing,
                        &rename_map,
                    )
                    .await;
                } else if let Ok(items) =
                    collection_repo::get_content_for_migration(&pool, &existing.id).await
                {
                    collection_repo::migrate_content_field_renames(&pool, &items, &rename_map)
                        .await;
                }
            }
        }
    }

    match collection_repo::update(&pool, &existing.id, &name, &new_slug, &definition_str).await {
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
pub async fn delete_collection(
    auth: AuthContext,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_write_access(&auth, &pool, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match collection_repo::delete(&pool, &site_id, &collection_slug).await {
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
