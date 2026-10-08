use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct CollectionSlug {
    collection_slug: String,
}

use crate::middleware::auth::{RequestContext, require_site_action};
use crate::models::authorization::Action;
use crate::models::collection::{CreateCollection, PublicCollection, UpdateCollection};
use crate::repository::Repository;
use crate::services::Services;

#[instrument(skip(repository, services, ctx))]
pub async fn list_collections(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SchemaRead).await {
        return (status, err).into_response();
    }

    match services.collection.list_collections(&ctx.site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/collections",
    responses((status = 200, description = "List of collection schemas", body = Vec<PublicCollection>)),
    security(("access_token" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn list_public_collections(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SchemaRead).await {
        return (status, err).into_response();
    }
    match services.collection.list_collections(&ctx.site_id).await {
        Ok(collections) => {
            let result: Result<Vec<_>, _> = collections.into_iter().map(PublicCollection::try_from).collect();
            match result {
                Ok(collections) => (StatusCode::OK, Json(collections)).into_response(),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "internal_error", "message": "Internal server error"})),
                )
                    .into_response(),
            }
        }
        Err(error) => error.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
pub async fn get_collection(
    ctx: RequestContext,
    Path(CollectionSlug { collection_slug }): Path<CollectionSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SchemaRead).await {
        return (status, err).into_response();
    }

    match services.collection.get_collection(&ctx.site_id, &collection_slug).await {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Collection not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/collections/{collection_slug}",
    params(CollectionSlug),
    responses((status = 200, description = "Collection schema", body = PublicCollection)),
    security(("access_token" = [])),
    tag = "collections"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn get_public_collection(
    ctx: RequestContext,
    Path(CollectionSlug { collection_slug }): Path<CollectionSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SchemaRead).await {
        return (status, err).into_response();
    }
    match services.collection.get_collection(&ctx.site_id, &collection_slug).await {
        Ok(Some(collection)) => match PublicCollection::try_from(collection) {
            Ok(collection) => (StatusCode::OK, Json(collection)).into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal_error", "message": "Internal server error"})),
            )
                .into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Collection not found"}))).into_response(),
        Err(error) => error.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
pub async fn create_collection(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateCollection>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SchemaWrite).await {
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

#[instrument(skip(repository, services, ctx, payload))]
pub async fn update_collection(
    ctx: RequestContext,
    Path(CollectionSlug { collection_slug }): Path<CollectionSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateCollection>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SchemaWrite).await {
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

#[instrument(skip(repository, services, ctx))]
pub async fn delete_collection(
    ctx: RequestContext,
    Path(CollectionSlug { collection_slug }): Path<CollectionSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SchemaWrite).await {
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
