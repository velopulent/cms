use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;

#[derive(Deserialize)]
pub struct CollectionSlug {
    collection_slug: String,
}

use crate::middleware::auth::{RequestContext, Scope, require_site_scope};
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
#[instrument(skip(repository, services, ctx))]
pub async fn list_collections(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::CollectionsRead, "viewer").await {
        return (status, err).into_response();
    }

    match services.collection.list_collections(&ctx.site_id).await {
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
#[instrument(skip(repository, services, ctx))]
pub async fn get_collection(
    ctx: RequestContext,
    Path(CollectionSlug { collection_slug }): Path<CollectionSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::CollectionsRead, "viewer").await {
        return (status, err).into_response();
    }

    match services
        .collection
        .get_collection(&ctx.site_id, &collection_slug)
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
#[instrument(skip(repository, services, ctx, payload))]
pub async fn create_collection(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateCollection>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::CollectionsWrite, "editor").await {
        return (status, err).into_response();
    }

    let definition_str = payload.definition.to_string();
    let is_singleton = payload.is_singleton.unwrap_or(false);

    match services
        .collection
        .create_collection(
            &ctx.site_id,
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
#[instrument(skip(repository, services, ctx, payload))]
pub async fn update_collection(
    ctx: RequestContext,
    Path(CollectionSlug { collection_slug }): Path<CollectionSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateCollection>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::CollectionsWrite, "editor").await {
        return (status, err).into_response();
    }

    let definition_str = payload.definition.as_ref().map(|s| s.to_string());

    match services
        .collection
        .update_collection(
            &ctx.site_id,
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
#[instrument(skip(repository, services, ctx))]
pub async fn delete_collection(
    ctx: RequestContext,
    Path(CollectionSlug { collection_slug }): Path<CollectionSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::CollectionsWrite, "editor").await {
        return (status, err).into_response();
    }

    match services
        .collection
        .delete_collection(&ctx.site_id, &collection_slug)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
