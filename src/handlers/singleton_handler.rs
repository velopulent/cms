use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::handlers::entry_handler::resolve_entries_files_from_value;
use crate::handlers::file_handler::StorageManager;
use crate::middleware::auth::{AuthContext, check_read_access_repo, check_write_access_repo};
use crate::models::collection::{SingletonResponse, UpdateSingletonData};
use crate::repository::Repository;

fn singleton_to_response(c: &crate::models::collection::Collection) -> SingletonResponse {
    let definition: serde_json::Value = serde_json::from_str(&c.definition).unwrap_or(json!({"fields": []}));
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

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/singletons",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 200, description = "List of singletons", body = Vec<SingletonResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, auth))]
pub async fn list_singletons(
    auth: AuthContext,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_read_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match repository.collection.list_singletons_only(&site_id).await {
        Ok(items) => {
            let responses: Vec<SingletonResponse> = items.iter().map(singleton_to_response).collect();
            (StatusCode::OK, Json(responses)).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/singletons/{slug}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("slug" = String, Path, description = "Singleton slug"),
    ),
    responses(
        (status = 200, description = "Singleton with data", body = SingletonResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Singleton not found"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, storage, auth))]
pub async fn get_singleton(
    auth: AuthContext,
    Path((site_id, slug)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    if let Err((status, err)) = check_read_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match repository.collection.get_by_slug(&site_id, &slug).await {
        Ok(Some(item)) => {
            if !item.is_singleton {
                return (StatusCode::NOT_FOUND, Json(json!({"error": "Singleton not found"}))).into_response();
            }

            let mut response = singleton_to_response(&item);

            if let Some(ref data) = response.data {
                let resolved = resolve_entries_files_from_value(data, &repository, &storage, &item.site_id).await;
                response.data = Some(resolved);
            }

            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Singleton not found"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}/singletons/{slug}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("slug" = String, Path, description = "Singleton slug"),
    ),
    request_body = UpdateSingletonData,
    responses(
        (status = 200, description = "Singleton data updated", body = SingletonResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Singleton not found"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, auth, payload))]
pub async fn update_singleton(
    auth: AuthContext,
    Path((site_id, slug)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateSingletonData>,
) -> Response {
    if let Err((status, err)) = check_write_access_repo(&auth, &repository, &site_id).await {
        return (status, Json(err)).into_response();
    }

    match repository.collection.get_by_slug(&site_id, &slug).await {
        Ok(Some(item)) => {
            if !item.is_singleton {
                return (StatusCode::NOT_FOUND, Json(json!({"error": "Singleton not found"}))).into_response();
            }

            let data_str = payload.data.to_string();

            match repository.collection.update_singleton_data(&item.id, &data_str).await {
                Ok(updated) => (StatusCode::OK, Json(singleton_to_response(&updated))).into_response(),
                Err(err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": err.to_string()})),
                )
                    .into_response(),
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Singleton not found"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}
