use axum::{
    Json,
    extract::{Extension, Path},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{HEADER_SITE_ID, Principal, SCOPE_SCHEMA_READ, SCOPE_SCHEMA_WRITE, require_site_scope};
use crate::models::collection::{Collection, CreateCollection, UpdateCollection};
use crate::repository::Repository;
use crate::services::Services;

#[utoipa::path(
    get,
    path = "/api/v1/collections",
    responses(
        (status = 200, description = "List of collections", body = Vec<Collection>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, services, principal))]
pub async fn list_collections(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site_id = headers.get(HEADER_SITE_ID).and_then(|v| v.to_str().ok());
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_SCHEMA_READ, "viewer").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.collection.list_collections(&site.site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/collections/{collection_slug}",
    params(("collection_slug" = String, Path, description = "Collection slug")),
    responses(
        (status = 200, description = "Collection details", body = Collection),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Collection not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, services, principal))]
pub async fn get_collection(
    principal: Principal,
    headers: HeaderMap,
    Path(collection_slug): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site_id = headers.get(HEADER_SITE_ID).and_then(|v| v.to_str().ok());
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_SCHEMA_READ, "viewer").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .collection
        .get_collection(&site.site_id, &collection_slug)
        .await
    {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Collection not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/collections",
    request_body = CreateCollection,
    responses(
        (status = 201, description = "Collection created", body = Collection),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 409, description = "Collection name or slug already exists"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, services, principal, payload))]
pub async fn create_collection(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateCollection>,
) -> Response {
    let site_id = headers.get(HEADER_SITE_ID).and_then(|v| v.to_str().ok());
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_SCHEMA_WRITE, "editor").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    let definition_str = payload.definition.to_string();
    let is_singleton = payload.is_singleton.unwrap_or(false);

    match services
        .collection
        .create_collection(
            &site.site_id,
            &payload.name,
            &payload.slug,
            &definition_str,
            is_singleton,
        )
        .await
    {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/collections/{collection_slug}",
    params(("collection_slug" = String, Path, description = "Collection slug")),
    request_body = UpdateCollection,
    responses(
        (status = 200, description = "Collection updated", body = Collection),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, services, principal, payload))]
pub async fn update_collection(
    principal: Principal,
    headers: HeaderMap,
    Path(collection_slug): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateCollection>,
) -> Response {
    let site_id = headers.get(HEADER_SITE_ID).and_then(|v| v.to_str().ok());
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_SCHEMA_WRITE, "editor").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    let definition_str = payload.definition.as_ref().map(|s| s.to_string());

    match services
        .collection
        .update_collection(
            &site.site_id,
            &collection_slug,
            payload.name.as_deref(),
            payload.slug.as_deref(),
            definition_str.as_deref(),
        )
        .await
    {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/collections/{collection_slug}",
    params(("collection_slug" = String, Path, description = "Collection slug")),
    responses(
        (status = 204, description = "Collection deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, services, principal))]
pub async fn delete_collection(
    principal: Principal,
    headers: HeaderMap,
    Path(collection_slug): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site_id = headers.get(HEADER_SITE_ID).and_then(|v| v.to_str().ok());
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_SCHEMA_WRITE, "editor").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .collection
        .delete_collection(&site.site_id, &collection_slug)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
