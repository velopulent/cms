use axum::{
    Json,
    extract::{Extension, Path},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bcrypt::{DEFAULT_COST, hash};
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use crate::middleware::auth::{
    HEADER_SITE_ID, Principal, SCOPE_TOKENS_READ, SCOPE_TOKENS_WRITE, default_instance_scopes, default_site_scopes,
    require_admin_scope, require_site_scope, scopes_to_string,
};
use crate::models::access_token::{AccessToken, AccessTokenKind, AccessTokenResponse, CreateInstanceToken, CreateSiteToken};
use crate::repository::Repository;

fn header_site_id(headers: &HeaderMap) -> Option<&str> {
    headers.get(HEADER_SITE_ID).and_then(|value| value.to_str().ok())
}

fn validate_scopes(kind: AccessTokenKind, requested: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let allowed = match kind {
        AccessTokenKind::Instance => default_instance_scopes()
            .into_iter()
            .map(ToString::to_string)
            .collect::<std::collections::BTreeSet<_>>(),
        AccessTokenKind::Site => default_site_scopes()
            .into_iter()
            .map(ToString::to_string)
            .collect::<std::collections::BTreeSet<_>>(),
    };

    let scopes = match requested {
        Some(scopes) if !scopes.is_empty() => scopes,
        _ => allowed.iter().cloned().collect(),
    };

    for scope in &scopes {
        if !allowed.contains(scope) {
            return Err(format!("Unsupported scope '{}'", scope));
        }
    }

    Ok(scopes)
}

fn build_token(kind: AccessTokenKind) -> String {
    let random_chars = Uuid::new_v4().to_string().replace('-', "");
    format!("{}{}", kind.prefix(), random_chars)
}

async fn create_token_record(
    repository: &Repository,
    config: &crate::config::Config,
    kind: AccessTokenKind,
    site_id: Option<&str>,
    name: String,
    scopes: Vec<String>,
    created_by_user_id: Option<&str>,
) -> Result<AccessTokenResponse, Response> {
    let raw_token = build_token(kind.clone());
    let prefix: String = raw_token.chars().take(24).collect();
    let token_hash = hash(&raw_token, DEFAULT_COST).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Hash error: {}", e)})),
        )
            .into_response()
    })?;
    let token_hmac = crate::middleware::auth::compute_key_hmac(&raw_token, &config.hmac_secret);
    let id = Uuid::now_v7().to_string();
    let scope_refs = scopes.iter().map(String::as_str).collect::<Vec<_>>();
    let scopes_string = scopes_to_string(&scope_refs);

    repository
        .access_token
        .create(
            &id,
            kind.clone(),
            site_id,
            &name,
            &token_hash,
            &prefix,
            &token_hmac,
            &scopes_string,
            created_by_user_id,
        )
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response()
        })?;

    Ok(AccessTokenResponse {
        id,
        kind: kind.to_string(),
        site_id: site_id.map(ToString::to_string),
        name,
        token: raw_token,
        token_prefix: prefix,
        scopes,
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/site-tokens",
    responses(
        (status = 200, description = "List site-scoped access tokens", body = Vec<AccessToken>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-tokens"
)]
#[instrument(skip(repository, principal))]
pub async fn list_site_tokens(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site_id = header_site_id(&headers);
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_TOKENS_READ, "admin").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match repository
        .access_token
        .list(AccessTokenKind::Site, Some(&site.site_id))
        .await
    {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site-tokens",
    request_body = CreateSiteToken,
    responses(
        (status = 201, description = "Site token created", body = AccessTokenResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-tokens"
)]
#[instrument(skip(repository, config, principal, payload))]
pub async fn create_site_token(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(config): Extension<crate::config::Config>,
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

    let scopes = match validate_scopes(AccessTokenKind::Site, payload.scopes) {
        Ok(scopes) => scopes,
        Err(err) => return (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    };

    match create_token_record(
        &repository,
        &config,
        AccessTokenKind::Site,
        Some(&site.site_id),
        payload.name,
        scopes,
        principal.user_id(),
    )
    .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(response) => response,
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
#[instrument(skip(repository, principal))]
pub async fn delete_site_token(
    principal: Principal,
    headers: HeaderMap,
    Path(token_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    let site_id = header_site_id(&headers);
    let site = match require_site_scope(&principal, &repository, site_id, SCOPE_TOKENS_WRITE, "admin").await {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match repository
        .access_token
        .delete(&token_id, AccessTokenKind::Site, Some(&site.site_id))
        .await
    {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Token not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/tokens",
    responses(
        (status = 200, description = "List instance-scoped access tokens", body = Vec<AccessToken>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "admin-tokens"
)]
#[instrument(skip(repository, principal))]
pub async fn list_instance_tokens(principal: Principal, Extension(repository): Extension<Repository>) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_TOKENS_READ).await {
        return (status, err).into_response();
    }

    match repository.access_token.list(AccessTokenKind::Instance, None).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/tokens",
    request_body = CreateInstanceToken,
    responses(
        (status = 201, description = "Instance token created", body = AccessTokenResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "admin-tokens"
)]
#[instrument(skip(repository, config, principal, payload))]
pub async fn create_instance_token(
    principal: Principal,
    Extension(repository): Extension<Repository>,
    Extension(config): Extension<crate::config::Config>,
    Json(payload): Json<CreateInstanceToken>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_TOKENS_WRITE).await {
        return (status, err).into_response();
    }

    if payload.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Name is required"}))).into_response();
    }

    let scopes = match validate_scopes(AccessTokenKind::Instance, payload.scopes) {
        Ok(scopes) => scopes,
        Err(err) => return (StatusCode::BAD_REQUEST, Json(json!({"error": err}))).into_response(),
    };

    match create_token_record(
        &repository,
        &config,
        AccessTokenKind::Instance,
        None,
        payload.name,
        scopes,
        principal.user_id(),
    )
    .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(response) => response,
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
#[instrument(skip(repository, principal))]
pub async fn delete_instance_token(
    principal: Principal,
    Path(token_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_TOKENS_WRITE).await {
        return (status, err).into_response();
    }

    match repository
        .access_token
        .delete(&token_id, AccessTokenKind::Instance, None)
        .await
    {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Token not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}
