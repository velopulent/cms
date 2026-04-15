use axum::{
    Json,
    extract::{Extension, Path},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::instrument;

use crate::middleware::auth::{HEADER_SITE_ID, Principal, SCOPE_CONTENT_READ, SCOPE_CONTENT_WRITE, require_site_scope};
use crate::models::collection::{SingletonResponse, UpdateSingletonData};
use crate::repository::Repository;
use crate::services::Services;

fn request_site_id(headers: &HeaderMap) -> Option<&str> {
    headers.get(HEADER_SITE_ID).and_then(|value| value.to_str().ok())
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
#[instrument(skip(repository, services, principal))]
pub async fn list_singletons(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site = match require_site_scope(
        &principal,
        &repository,
        request_site_id(&headers),
        SCOPE_CONTENT_READ,
        "viewer",
    )
    .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.singleton.list_singletons(&site.site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(e) => e.into_response(),
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
#[instrument(skip(repository, services, principal))]
pub async fn get_singleton(
    principal: Principal,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site = match require_site_scope(
        &principal,
        &repository,
        request_site_id(&headers),
        SCOPE_CONTENT_READ,
        "viewer",
    )
    .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.singleton.get_singleton(&site.site_id, &slug).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => e.into_response(),
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
#[instrument(skip(repository, services, principal, payload))]
pub async fn update_singleton(
    principal: Principal,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateSingletonData>,
) -> Response {
    let site = match require_site_scope(
        &principal,
        &repository,
        request_site_id(&headers),
        SCOPE_CONTENT_WRITE,
        "editor",
    )
    .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .singleton
        .update_singleton(&site.site_id, &slug, &payload.data)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}
