use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::HeaderMap,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{
    HEADER_SITE_ID, Principal, SCOPE_CONTENT_READ, SCOPE_CONTENT_WRITE, require_site_scope,
};
use crate::models::entry::{CreateEntry, Entry, UpdateEntry};
use crate::repository::Repository;
use crate::repository::traits::ListEntriesParams;
use crate::services::Services;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListParams {
    pub r#type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

fn request_site_id(headers: &HeaderMap) -> Option<&str> {
    headers.get(HEADER_SITE_ID).and_then(|value| value.to_str().ok())
}

#[utoipa::path(
    get,
    path = "/api/v1/entries",
    params(ListParams),
    responses(
        (status = 200, description = "List of entries"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, principal, params))]
pub async fn list_entries(
    principal: Principal,
    headers: HeaderMap,
    Query(params): Query<ListParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    let published_only = matches!(principal, Principal::SiteToken { .. });
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);

    let list_params = ListEntriesParams {
        site_id: &site.site_id,
        collection_slug: params.r#type.as_deref(),
        collection_id: None,
        status: if matches!(principal, Principal::UserSession { .. }) {
            params.status.as_deref()
        } else {
            None
        },
        search: params.search.as_deref(),
        published_only,
        page,
        per_page,
    };

    match services.entry.list_entries(list_params).await {
        Ok(result) => {
            let items = services.entry.resolve_entries_list_files(&result.items).await;
            (
                StatusCode::OK,
                Json(json!({
                    "items": items,
                    "total": result.total,
                    "page": result.page,
                    "per_page": result.per_page,
                })),
            )
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/site/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Entry", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, principal))]
pub async fn get_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_READ, "viewer")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    let published_only = matches!(principal, Principal::SiteToken { .. });

    match services.entry.get_entry(&id, &site.site_id, published_only).await {
        Ok(Some(item)) => {
            let resolved = services.entry.resolve_entry_files(&item).await.unwrap_or_else(|_| {
                serde_json::from_str(&item.data).unwrap_or_default()
            });
            (StatusCode::OK, Json(resolved)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/entries",
    request_body = CreateEntry,
    responses(
        (status = 201, description = "Entry created", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 409, description = "Slug already exists"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, principal, payload))]
pub async fn create_entry(
    principal: Principal,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateEntry>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .entry
        .create_entry(&site.site_id, &payload.collection_id, &payload.data, &payload.slug)
        .await
    {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/site/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    request_body = UpdateEntry,
    responses(
        (status = 200, description = "Entry updated", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, principal, payload))]
pub async fn update_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateEntry>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .entry
        .update_entry(&id, &site.site_id, payload.data.as_ref(), payload.slug.as_deref(), payload.status.as_deref())
        .await
    {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/site/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 204, description = "Entry deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, principal))]
pub async fn delete_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.entry.delete_entry(&id, &site.site_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/entries/{id}/publish",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Entry published", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, principal))]
pub async fn publish_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.entry.publish_entry(&id, &site.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/site/entries/{id}/unpublish",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Entry unpublished", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, principal))]
pub async fn unpublish_entry(
    principal: Principal,
    headers: HeaderMap,
    Path(id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let site = match require_site_scope(&principal, &repository, request_site_id(&headers), SCOPE_CONTENT_WRITE, "editor")
        .await
    {
        Ok(site) => site,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services.entry.unpublish_entry(&id, &site.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}
