use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::instrument;

#[derive(Deserialize)]
pub struct SingletonSlug {
    slug: String,
}

use crate::middleware::auth::{RequestContext, require_site_action};
use crate::models::authorization::Action;
use crate::models::collection::{SingletonResponse, UpdateSingletonData};
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::{StorageProvider, StorageRegistry};

fn get_storage_for_site(
    site_storage_provider: &str,
    registry: &StorageRegistry,
) -> Result<Arc<dyn StorageProvider>, Response> {
    registry.get(site_storage_provider).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Storage not configured"})),
        )
            .into_response()
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/singletons",
    responses(
        (status = 200, description = "List of singletons", body = Vec<SingletonResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn list_singletons(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentRead).await {
        return (status, err).into_response();
    }

    match services.singleton.list_singletons(&ctx.site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/singletons/{slug}",
    params(("slug" = String, Path, description = "Singleton slug")),
    responses(
        (status = 200, description = "Singleton with data", body = SingletonResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Singleton not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "singletons"
)]
#[instrument(skip(repository, services, ctx, storage_registry))]
pub async fn get_singleton(
    ctx: RequestContext,
    Path(SingletonSlug { slug }): Path<SingletonSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentRead).await {
        return (status, err).into_response();
    }

    let storage_provider = services
        .file
        .get_storage_provider(&ctx.site_id)
        .await
        .unwrap_or_else(|_| "filesystem".into());
    let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    match services.singleton.get_singleton(&ctx.site_id, &slug, storage).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/singletons/{slug}",
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
#[instrument(skip(repository, services, ctx, payload))]
pub async fn update_singleton(
    ctx: RequestContext,
    Path(SingletonSlug { slug }): Path<SingletonSlug>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateSingletonData>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    match services
        .singleton
        .update_singleton(
            &ctx.site_id,
            &slug,
            &payload.data,
            ctx.auth.actor.user_id(),
            payload.change_summary.as_deref(),
        )
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}
