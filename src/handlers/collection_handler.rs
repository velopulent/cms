use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::middleware::auth::{AuthContext, AuthenticatedUser, check_site_access};
use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::models::content::Content;

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
    match &auth {
        AuthContext::Jwt { user_id } => {
            if let Err((status, err)) = check_site_access(&pool, user_id, &site_id, "viewer").await
            {
                return (status, Json(err)).into_response();
            }
        }
        AuthContext::ApiKey {
            site_id: key_site_id,
        } => {
            if key_site_id != &site_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "API key does not have access to this site"})),
                )
                    .into_response();
            }
        }
    }

    let result = sqlx::query_as::<_, Collection>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? ORDER BY name",
    )
    .bind(&site_id)
    .fetch_all(&pool)
    .await;

    match result {
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
    match &auth {
        AuthContext::Jwt { user_id } => {
            if let Err((status, err)) = check_site_access(&pool, user_id, &site_id, "viewer").await
            {
                return (status, Json(err)).into_response();
            }
        }
        AuthContext::ApiKey {
            site_id: key_site_id,
        } => {
            if key_site_id != &site_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "API key does not have access to this site"})),
                )
                    .into_response();
            }
        }
    }

    let result = sqlx::query_as::<_, Collection>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
    )
    .bind(&site_id)
    .bind(&collection_slug)
    .fetch_optional(&pool)
    .await;

    match result {
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
    security(("bearer" = [])),
    tag = "collections"
)]
pub async fn create_collection(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateCollection>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let definition_str = payload.definition.to_string();
    let id = Uuid::now_v7().to_string();

    let result = sqlx::query(
        "INSERT INTO collections (id, site_id, name, slug, definition) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&site_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&definition_str)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let item = sqlx::query_as::<_, Collection>(
                "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE id = ?",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::CREATED, Json(item)).into_response()
        }
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
    security(("bearer" = [])),
    tag = "collections"
)]
pub async fn update_collection(
    auth: AuthenticatedUser,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateCollection>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let existing = sqlx::query_as::<_, Collection>(
        "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE site_id = ? AND slug = ?",
    )
    .bind(&site_id)
    .bind(&collection_slug)
    .fetch_optional(&pool)
    .await;

    let existing = match existing {
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

    let name = payload.name.unwrap_or(existing.name);
    let new_slug = payload.slug.unwrap_or(existing.slug);
    let definition_str = payload
        .definition
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| existing.definition.clone());

    if let Some(ref new_def_value) = payload.definition {
        let old_def: Option<serde_json::Value> = serde_json::from_str(&existing.definition).ok();
        let new_def: Option<serde_json::Value> = serde_json::from_value(new_def_value.clone()).ok();

        if let (Some(old_d), Some(new_d)) = (old_def, new_def) {
            let old_fields = old_d["fields"].as_array().cloned().unwrap_or_default();
            let new_fields = new_d["fields"].as_array().cloned().unwrap_or_default();

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

            if !rename_map.is_empty() {
                let contents = sqlx::query_as::<_, Content>(
                    "SELECT id, site_id, collection_id, data, slug, status, created_at, updated_at, published_at FROM content WHERE collection_id = ?",
                )
                .bind(&existing.id)
                .fetch_all(&pool)
                .await;

                if let Ok(items) = contents {
                    for content in &items {
                        if let Ok(mut data) =
                            serde_json::from_str::<serde_json::Value>(&content.data)
                        {
                            if let Some(obj) = data.as_object_mut() {
                                let mut renamed = serde_json::Map::new();
                                for (key, value) in obj.iter() {
                                    let new_key =
                                        rename_map.get(key).cloned().unwrap_or_else(|| key.clone());
                                    renamed.insert(new_key, value.clone());
                                }
                                let new_data = serde_json::Value::Object(renamed);
                                let new_data_str = serde_json::to_string(&new_data)
                                    .unwrap_or_else(|_| content.data.clone());

                                let _ = sqlx::query(
                                    "UPDATE content SET data = ?, updated_at = datetime('now') WHERE id = ?",
                                )
                                .bind(&new_data_str)
                                .bind(&content.id)
                                .execute(&pool)
                                .await;
                            }
                        }
                    }
                }
            }
        }
    }

    let result = sqlx::query(
        "UPDATE collections SET name = ?, slug = ?, definition = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&name)
    .bind(&new_slug)
    .bind(&definition_str)
    .bind(&existing.id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let item = sqlx::query_as::<_, Collection>(
                "SELECT id, site_id, name, slug, definition, created_at, updated_at FROM collections WHERE id = ?",
            )
            .bind(&existing.id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(item)).into_response()
        }
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
    security(("bearer" = [])),
    tag = "collections"
)]
pub async fn delete_collection(
    auth: AuthenticatedUser,
    Path((site_id, collection_slug)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "editor").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query("DELETE FROM collections WHERE site_id = ? AND slug = ?")
        .bind(&site_id)
        .bind(&collection_slug)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}
