use axum::{
    Json,
    extract::{Extension, Path},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{
    HEADER_SITE_ID, Principal, SCOPE_TOKENS_READ, SCOPE_TOKENS_WRITE, require_admin_scope, require_site_scope,
};
use crate::models::access_token::{CreateInstanceToken, CreateSiteToken};
use crate::repository::Repository;
use crate::services::Services;

fn header_site_id(headers: &HeaderMap) -> Option<&str> {
    headers.get(HEADER_SITE_ID).and_then(|value| value.to_str().ok())
}

#[utoipa::path(
    get,
    path = "/api/v1/site-tokens",
    responses(
        (status = 200, description = "List site-scoped access tokens"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-tokens"
)]
#[instrument(skip(repository, services, principal))]
pub async fn list_site_tokens(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site_id = header_site_id(&headers);
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_TOKENS_READ, "admin").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.access_token.list_site_tokens(&site.site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site-tokens",
    request_body = CreateSiteToken,
    responses(
        (status = 201, description = "Site token created"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-tokens"
)]
#[instrument(skip(repository, services, principal, payload))]
pub async fn create_site_token(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateSiteToken>,
) -> Response {
    let site_id = header_site_id(&headers);
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_TOKENS_WRITE, "admin").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    if payload.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Name is required"}))).into_response();
    }

    let scopes = payload.scopes.unwrap_or_default();

    match services
        .access_token
        .create_site_token(&site.site_id, payload.name, scopes, principal.user_id())
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/site-tokens/{token_id}",
    params(("token_id" = String, Path, description = "Site token id")),
    responses(
        (status = 204, description = "Site token deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Token not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-tokens"
)]
#[instrument(skip(repository, services, principal))]
pub async fn delete_site_token(
    principal: Principal,
    headers: HeaderMap,
    Path(token_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site_id = header_site_id(&headers);
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_TOKENS_WRITE, "admin").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.access_token.delete_site_token(&token_id, &site.site_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Token not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/tokens",
    responses(
        (status = 200, description = "List instance-scoped access tokens"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "admin-tokens"
)]
#[instrument(skip(repository, services, principal))]
pub async fn list_instance_tokens(
    principal: Principal,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_TOKENS_READ).await {
        return (status, err).into_response();
    }

    match services.access_token.list_instance_tokens().await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/tokens",
    request_body = CreateInstanceToken,
    responses(
        (status = 201, description = "Instance token created"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "admin-tokens"
)]
#[instrument(skip(repository, services, principal, payload))]
pub async fn create_instance_token(
    principal: Principal,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateInstanceToken>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_TOKENS_WRITE).await {
        return (status, err).into_response();
    }

    if payload.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Name is required"}))).into_response();
    }

    let scopes = payload.scopes.unwrap_or_default();

    match services.access_token.create_instance_token(payload.name, scopes).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/tokens/{token_id}",
    params(("token_id" = String, Path, description = "Instance token id")),
    responses(
        (status = 204, description = "Instance token deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Token not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "admin-tokens"
)]
#[instrument(skip(repository, services, principal))]
pub async fn delete_instance_token(
    principal: Principal,
    Path(token_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_TOKENS_WRITE).await {
        return (status, err).into_response();
    }

    match services.access_token.delete_instance_token(&token_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Token not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
