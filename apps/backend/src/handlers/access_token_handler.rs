use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{Actor, AuthContext, RequestContext, require_user_action};
use crate::models::access_token::{CreatePersonalAccessToken, CreateSiteToken};
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
        .create_site_token(&site_id, payload.name, payload.scopes, Some(user_id))
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn list_personal_tokens(auth: AuthContext, Extension(services): Extension<Services>) -> Response {
    let Actor::User(user) = auth.actor else {
        return (StatusCode::FORBIDDEN, Json(json!({"error":"session_required"}))).into_response();
    };
    match services.access_token.list_personal_tokens(&user.user_id).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn create_personal_token(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreatePersonalAccessToken>,
) -> Response {
    let Actor::User(user) = auth.actor else {
        return (StatusCode::FORBIDDEN, Json(json!({"error":"session_required"}))).into_response();
    };
    let operator = repository
        .user
        .find_by_id(&user.user_id)
        .await
        .ok()
        .flatten()
        .and_then(|account| account.instance_role)
        .is_some();
    if !operator
        && payload.scopes.iter().any(|scope| {
            matches!(
                scope,
                crate::models::access_token::TokenScope::SiteSettingsWrite
                    | crate::models::access_token::TokenScope::SchemaWrite
                    | crate::models::access_token::TokenScope::WebhooksRead
                    | crate::models::access_token::TokenScope::WebhooksWrite
                    | crate::models::access_token::TokenScope::WebhooksTrigger
                    | crate::models::access_token::TokenScope::DeploymentsWrite
            )
        })
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"scope_exceeds_current_role"})),
        )
            .into_response();
    }
    match services
        .access_token
        .create_personal_token(&user.user_id, payload)
        .await
    {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn revoke_personal_token(
    auth: AuthContext,
    Path(token_id): Path<String>,
    Extension(services): Extension<Services>,
) -> Response {
    let Actor::User(user) = auth.actor else {
        return (StatusCode::FORBIDDEN, Json(json!({"error":"session_required"}))).into_response();
    };
    match services
        .access_token
        .revoke_personal_token(&token_id, &user.user_id)
        .await
    {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
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
