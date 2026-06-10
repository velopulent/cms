use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{RequestContext, require_user_action};
use crate::models::access_token::CreateSiteToken;
use crate::models::authorization::Action;
use crate::repository::Repository;
use crate::services::Services;

#[instrument(skip(repository, services, ctx))]
pub async fn list_site_tokens(
    ctx: RequestContext,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_user_action(&ctx, &repository, Action::ApiKeysManage).await {
        return (status, err).into_response();
    }

    match services.access_token.list_site_tokens(&site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
pub async fn create_site_token(
    ctx: RequestContext,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateSiteToken>,
) -> Response {
    if let Err((status, err)) = require_user_action(&ctx, &repository, Action::ApiKeysManage).await {
        return (status, err).into_response();
    }

    if payload.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Name is required"}))).into_response();
    }

    let user_id = ctx.auth.actor.user_id().unwrap_or("system");

    match services
        .access_token
        .create_site_token(&site_id, payload.name, payload.permission, Some(user_id))
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
pub async fn delete_site_token(
    ctx: RequestContext,
    Path((site_id, token_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_user_action(&ctx, &repository, Action::ApiKeysManage).await {
        return (status, err).into_response();
    }

    match services.access_token.delete_site_token(&token_id, &site_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Token not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
