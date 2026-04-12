use axum::{
    Json,
    extract::{Extension, Path},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::handlers::entry_handler::resolve_entries_files_from_value;
use crate::handlers::file_handler::StorageManager;
use crate::middleware::auth::{
    HEADER_SITE_ID, Principal, SCOPE_CONTENT_READ, SCOPE_CONTENT_WRITE, require_site_scope,
};
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

fn request_site_id(headers: &HeaderMap) -> Option<&str> {
    headers.get(HEADER_SITE_ID).and_then(|value| value.to_str().ok())
}

#[utoipa::path(
    get,
    path = "/api/v1/site/singletons",
    responses(
        (status = 200, description = "List of singletons", body = Vec<SingletonResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, principal))]
pub async fn list_singletons(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.collection.list_singletons_only(&site.site_id).await {
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
    path = "/api/v1/site/singletons/{slug}",
    params(("slug" = String, Path, description = "Singleton slug")),
    responses(
        (status = 200, description = "Singleton with data", body = SingletonResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Singleton not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, storage, principal))]
pub async fn get_singleton(
    principal: Principal,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(storage): Extension<StorageManager>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.collection.get_by_slug(&site.site_id, &slug).await {
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
    path = "/api/v1/site/singletons/{slug}",
    params(("slug" = String, Path, description = "Singleton slug")),
    request_body = UpdateSingletonData,
    responses(
        (status = 200, description = "Singleton data updated", body = SingletonResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Singleton not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, principal, payload))]
pub async fn update_singleton(
    principal: Principal,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateSingletonData>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, Json(err)).into_response(),
    };

    match repository.collection.get_by_slug(&site.site_id, &slug).await {
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
