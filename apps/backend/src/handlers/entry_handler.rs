use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::instrument;

use crate::config::Config;
#[derive(Deserialize, utoipa::IntoParams)]
pub struct EntryId {
    id: String,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct RevisionId {
    id: String,
    number: i64,
}

use crate::error::AppError;
use crate::middleware::auth::{Actor, RequestContext, require_site_action};
use crate::models::authorization::Action;
use crate::models::entry::{
    CreateEntry, Entry, EntryRevisionResponse, PublicEntry, RevisionsListResponse, UpdateEntry,
};
use crate::repository::Repository;
use crate::repository::traits::ListEntriesParams;
use crate::services::Services;
use crate::services::entry::UpdateEntryInput;
use crate::storage::{StorageProvider, StorageRegistry};
use crate::utils::diff::compute_diff_for_revision;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListParams {
    pub r#type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub include_drafts: Option<bool>,
    pub cursor: Option<String>,
    pub include_total: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateCollectionEntry {
    pub data: serde_json::Value,
    pub slug: String,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct CollectionEntryPath {
    pub collection_slug: String,
}

#[derive(Deserialize, utoipa::IntoParams, Debug)]
pub struct RevisionListParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Deserialize, utoipa::IntoParams, Debug)]
pub struct DiffQuery {
    pub diff: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PreviewQuery {
    pub include_drafts: Option<bool>,
}

pub(crate) fn with_etag(mut response: Response, version: &str) -> Response {
    if let Ok(value) = format!("\"{version}\"").parse() {
        response.headers_mut().insert(header::ETAG, value);
    }
    response
}

pub(crate) fn if_match_version(headers: &HeaderMap) -> Result<Option<&str>, AppError> {
    let Some(header) = headers.get(header::IF_MATCH) else {
        return Ok(None);
    };
    let value = header
        .to_str()
        .map_err(|_| AppError::BadRequest("Invalid If-Match header".into()))?
        .trim();
    if value == "*" {
        return Ok(None);
    }
    let version = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte == 0x21 || (0x23..=0x7e).contains(&byte)))
        .ok_or_else(|| AppError::BadRequest("If-Match requires one strong quoted ETag or *".into()))?;
    Ok(Some(version))
}

fn get_storage_for_site(
    site_storage_provider: &str,
    registry: &StorageRegistry,
) -> Result<Arc<dyn StorageProvider>, AppError> {
    registry
        .get(site_storage_provider)
        .ok_or(AppError::Internal("Storage provider not found".into()))
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
#[instrument(skip(repository, services, ctx, params))]
pub async fn list_entries(
    ctx: RequestContext,
    Query(params): Query<ListParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let revision_read_action = if matches!(ctx.auth.actor, Actor::User(_)) {
        Action::ContentRead
    } else {
        Action::ContentPreviewRead
    };
    if let Err((status, err)) = require_site_action(&ctx, &repository, revision_read_action).await {
        return (status, err).into_response();
    }

    let published_only = matches!(ctx.auth.actor, Actor::ApiKey(_));
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);

    let list_params = ListEntriesParams {
        site_id: &ctx.site_id,
        collection_slug: params.r#type.as_deref(),
        collection_id: None,
        status: if matches!(ctx.auth.actor, Actor::User(_)) {
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

/// Public resource-oriented variant. Collection is carried in the path so the
/// external API never needs an implementation-specific collection id.
#[instrument(skip(repository, services, ctx, params))]
#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/collections/{collection_slug}/entries",
    params(CollectionEntryPath, ListParams),
    responses((status = 200, description = "Entries in a collection")),
    security(("access_token" = [])),
    tag = "entries"
)]
pub async fn list_collection_entries(
    ctx: RequestContext,
    Path(CollectionEntryPath { collection_slug }): Path<CollectionEntryPath>,
    Query(params): Query<ListParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(config): Extension<Config>,
) -> Response {
    if let Err((status, err)) = require_site_action(
        &ctx,
        &repository,
        if params.include_drafts.unwrap_or(false) || params.status.as_deref() == Some("draft") {
            Action::ContentPreviewRead
        } else {
            Action::ContentRead
        },
    )
    .await
    {
        return (status, err).into_response();
    }

    let include_drafts = params.include_drafts.unwrap_or(false) || params.status.as_deref() == Some("draft");
    let fingerprint = crate::utils::cursor::fingerprint(&(
        &ctx.site_id,
        &collection_slug,
        &params.status,
        &params.search,
        include_drafts,
        params.per_page.unwrap_or(50).clamp(1, 200),
    ));
    let page = match params.cursor.as_deref() {
        Some(cursor) => match crate::utils::cursor::decode(cursor, &config.token_index_key) {
            Ok(cursor) if cursor.fingerprint == fingerprint => cursor.page,
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "invalid_cursor", "message": "Cursor does not match this query"})),
                )
                    .into_response();
            }
        },
        None => params.page.unwrap_or(1).max(1),
    };
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);
    let result = services
        .entry
        .list_entries(ListEntriesParams {
            site_id: &ctx.site_id,
            collection_slug: Some(&collection_slug),
            collection_id: None,
            status: params.status.as_deref(),
            search: params.search.as_deref(),
            published_only: !include_drafts,
            page,
            per_page,
        })
        .await;

    match result {
        Ok(result) => {
            let items: Result<Vec<_>, _> = result.items.into_iter().map(PublicEntry::try_from).collect();
            match items {
                Ok(items) => {
                    let has_next_page = result.page.saturating_mul(result.per_page) < result.total;
                    let next_cursor = has_next_page.then(|| {
                        crate::utils::cursor::encode(
                            &crate::utils::cursor::PageCursor {
                                version: 1,
                                page: result.page + 1,
                                fingerprint,
                            },
                            &config.token_index_key,
                        )
                    });
                    (
                        StatusCode::OK,
                        Json(json!({
                            "items": items,
                            "page_info": {
                                "has_next_page": has_next_page,
                                "next_cursor": next_cursor,
                            },
                            "total": params.include_total.unwrap_or(false).then_some(result.total),
                            "per_page": result.per_page,
                        })),
                    )
                        .into_response()
                }
                Err(error) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "invalid_stored_entry", "message": error})),
                )
                    .into_response(),
            }
        }
        Err(error) => error.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 200, description = "Entry", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, ctx, storage_registry))]
pub async fn get_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Extension(storage_registry): Extension<Arc<StorageRegistry>>,
) -> Response {
    let revision_read_action = if matches!(ctx.auth.actor, Actor::User(_)) {
        Action::ContentRead
    } else {
        Action::ContentPreviewRead
    };
    if let Err((status, err)) = require_site_action(&ctx, &repository, revision_read_action).await {
        return (status, err).into_response();
    }

    let published_only = matches!(ctx.auth.actor, Actor::ApiKey(_));

    match services.entry.get_entry(&id, &ctx.site_id, published_only).await {
        Ok(Some(item)) => {
            let storage_provider = match services.file.get_storage_provider(&ctx.site_id).await {
                Ok(provider) => provider,
                Err(error) => return error.into_response(),
            };
            let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
                Ok(s) => s,
                Err(e) => return e.into_response(),
            };
            let resolved = services
                .entry
                .resolve_entry_files(&item, storage)
                .await
                .unwrap_or_else(|_| serde_json::from_str(&item.data).unwrap_or_default());
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
#[instrument(skip(repository, services, ctx, payload))]
pub async fn create_entry(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateEntry>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    let created_by = ctx.auth.actor.user_id();
    match services
        .entry
        .create_entry(
            &ctx.site_id,
            &payload.collection_id,
            &payload.data,
            &payload.slug,
            created_by,
        )
        .await
    {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/collections/{collection_slug}/entries",
    params(CollectionEntryPath),
    request_body = CreateCollectionEntry,
    responses((status = 201, description = "Entry created", body = PublicEntry)),
    security(("access_token" = [])),
    tag = "entries"
)]
pub async fn create_collection_entry(
    ctx: RequestContext,
    Path(CollectionEntryPath { collection_slug }): Path<CollectionEntryPath>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    axum::Json(payload): axum::Json<CreateCollectionEntry>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    let collection = match services.collection.get_collection(&ctx.site_id, &collection_slug).await {
        Ok(Some(collection)) => collection,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Collection not found"}))).into_response(),
        Err(error) => return error.into_response(),
    };
    if collection.is_singleton {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "Use the singleton resource for singleton collections"})),
        )
            .into_response();
    }

    match services
        .entry
        .create_entry(
            &ctx.site_id,
            &collection.id,
            &payload.data,
            &payload.slug,
            ctx.auth.actor.user_id(),
        )
        .await
    {
        Ok(item) => match PublicEntry::try_from(item) {
            Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "invalid_stored_entry", "message": error})),
            )
                .into_response(),
        },
        Err(error) => error.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/entries/{id}",
    params(EntryId),
    responses((status = 200, description = "Entry", body = PublicEntry)),
    security(("access_token" = [])),
    tag = "entries"
)]
pub async fn get_public_entry(
    ctx: RequestContext,
    Query(params): Query<PreviewQuery>,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let include_drafts = params.include_drafts.unwrap_or(false);
    if let Err((status, err)) = require_site_action(
        &ctx,
        &repository,
        if include_drafts {
            Action::ContentPreviewRead
        } else {
            Action::ContentRead
        },
    )
    .await
    {
        return (status, err).into_response();
    }
    match services.entry.get_entry(&id, &ctx.site_id, !include_drafts).await {
        Ok(Some(entry)) => match PublicEntry::try_from(entry) {
            Ok(entry) => with_etag((StatusCode::OK, Json(entry.clone())).into_response(), &entry.version),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "invalid_stored_entry", "message": error})),
            )
                .into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response(),
        Err(error) => error.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
#[utoipa::path(
    patch,
    path = "/api/v1/sites/{site_id}/entries/{id}",
    params(EntryId),
    request_body = UpdateEntry,
    responses((status = 200, description = "Entry updated", body = PublicEntry)),
    security(("access_token" = [])),
    tag = "entries"
)]
pub async fn update_public_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    headers: HeaderMap,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateEntry>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }
    if payload.status.is_some()
        && let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentPublish).await
    {
        return (status, err).into_response();
    }
    let expected_version = match if_match_version(&headers) {
        Ok(version) => version.or(payload.expected_version.as_deref()),
        Err(error) => return error.into_response(),
    };
    match services
        .entry
        .update_entry(UpdateEntryInput {
            id: &id,
            site_id: &ctx.site_id,
            data: payload.data.as_ref(),
            slug: payload.slug.as_deref(),
            status: payload.status.as_deref(),
            created_by: ctx.auth.actor.user_id(),
            change_summary: payload.change_summary.as_deref(),
            expected_version,
        })
        .await
    {
        Ok(entry) => match PublicEntry::try_from(entry) {
            Ok(entry) => with_etag((StatusCode::OK, Json(entry.clone())).into_response(), &entry.version),
            Err(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "invalid_stored_entry", "message": error})),
            )
                .into_response(),
        },
        Err(error) => error.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/entries/{id}/publish",
    params(EntryId),
    responses((status = 200, description = "Entry published", body = PublicEntry)),
    security(("access_token" = [])),
    tag = "entries"
)]
pub async fn publish_public_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentPublish).await {
        return (status, err).into_response();
    }
    match services.entry.publish_entry(&id, &ctx.site_id).await {
        Ok(entry) => match PublicEntry::try_from(entry) {
            Ok(entry) => with_etag((StatusCode::OK, Json(entry.clone())).into_response(), &entry.version),
            Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": error}))).into_response(),
        },
        Err(error) => error.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/entries/{id}/unpublish",
    params(EntryId),
    responses((status = 200, description = "Entry unpublished", body = PublicEntry)),
    security(("access_token" = [])),
    tag = "entries"
)]
pub async fn unpublish_public_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentPublish).await {
        return (status, err).into_response();
    }
    match services.entry.unpublish_entry(&id, &ctx.site_id).await {
        Ok(entry) => match PublicEntry::try_from(entry) {
            Ok(entry) => with_etag((StatusCode::OK, Json(entry.clone())).into_response(), &entry.version),
            Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": error}))).into_response(),
        },
        Err(error) => error.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}/entries/{id}",
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
#[instrument(skip(repository, services, ctx, payload))]
pub async fn update_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateEntry>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    let created_by = ctx.auth.actor.user_id();
    match services
        .entry
        .update_entry(UpdateEntryInput {
            id: &id,
            site_id: &ctx.site_id,
            data: payload.data.as_ref(),
            slug: payload.slug.as_deref(),
            status: payload.status.as_deref(),
            created_by,
            change_summary: payload.change_summary.as_deref(),
            expected_version: None,
        })
        .await
    {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/entries/{id}",
    params(("id" = String, Path, description = "Entry ID")),
    responses(
        (status = 204, description = "Entry deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn delete_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    match services.entry.delete_entry(&id, &ctx.site_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/entries/{id}/publish",
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
#[instrument(skip(repository, services, ctx))]
pub async fn publish_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    match services.entry.publish_entry(&id, &ctx.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/entries/{id}/unpublish",
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
#[instrument(skip(repository, services, ctx))]
pub async fn unpublish_entry(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    match services.entry.unpublish_entry(&id, &ctx.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/entries/{id}/revisions",
    params(("id" = String, Path, description = "Entry ID"), RevisionListParams),
    responses(
        (status = 200, description = "List of revisions", body = RevisionsListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Entry not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn list_entry_revisions(
    ctx: RequestContext,
    Path(EntryId { id }): Path<EntryId>,
    Query(params): Query<RevisionListParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentPreviewRead).await {
        return (status, err).into_response();
    }

    // Verify entry exists and belongs to site
    match services.entry.get_entry(&id, &ctx.site_id, false).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response(),
        Err(e) => return e.into_response(),
    }

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);

    match services.entry.list_revisions(&id, &ctx.site_id, page, per_page).await {
        Ok(result) => {
            let response = RevisionsListResponse {
                items: result.items.into_iter().map(EntryRevisionResponse::from).collect(),
                total: result.total,
                page: result.page,
                per_page: result.per_page,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/entries/{id}/revisions/{number}",
    params(
        ("id" = String, Path, description = "Entry ID"),
        ("number" = i64, Path, description = "Revision number"),
        DiffQuery
    ),
    responses(
        (status = 200, description = "Revision", body = EntryRevisionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Revision not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn get_entry_revision(
    ctx: RequestContext,
    Path(RevisionId { id, number }): Path<RevisionId>,
    Query(query): Query<DiffQuery>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentPreviewRead).await {
        return (status, err).into_response();
    }

    // Verify entry exists and belongs to site
    match services.entry.get_entry(&id, &ctx.site_id, false).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Entry not found"}))).into_response(),
        Err(e) => return e.into_response(),
    }

    match services.entry.get_revision(&id, &ctx.site_id, number).await {
        Ok(Some(revision)) => {
            let mut response = EntryRevisionResponse::from(revision.clone());

            if query.diff.unwrap_or(false)
                && number > 1
                && let Ok(Some(prev)) = services.entry.get_revision(&id, &ctx.site_id, number - 1).await
                && let Some(diff) = compute_diff_for_revision(&revision, Some(&prev))
            {
                response.diff_from_previous = Some(diff);
            }

            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Revision not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/entries/{id}/revisions/{number}/restore",
    params(
        ("id" = String, Path, description = "Entry ID"),
        ("number" = i64, Path, description = "Revision number"),
    ),
    responses(
        (status = 200, description = "Entry restored", body = Entry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Revision not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn restore_entry_revision(
    ctx: RequestContext,
    Path(RevisionId { id, number }): Path<RevisionId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, err).into_response();
    }

    let created_by = ctx.auth.actor.user_id();
    match services
        .entry
        .restore_revision(&id, &ctx.site_id, number, created_by)
        .await
    {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/entries/{id}/revisions/{number}/restore",
    params(
        ("id" = String, Path, description = "Entry ID"),
        ("number" = i64, Path, description = "Revision number"),
    ),
    responses(
        (status = 200, description = "Entry restored", body = PublicEntry),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Revision not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "entries"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn restore_public_entry_revision(
    ctx: RequestContext,
    Path(RevisionId { id, number }): Path<RevisionId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, error)) = require_site_action(&ctx, &repository, Action::ContentWrite).await {
        return (status, error).into_response();
    }
    match services
        .entry
        .restore_revision(&id, &ctx.site_id, number, ctx.auth.actor.user_id())
        .await
    {
        Ok(entry) => match PublicEntry::try_from(entry) {
            Ok(entry) => {
                let version = entry.version.clone();
                with_etag((StatusCode::OK, Json(entry)).into_response(), &version)
            }
            Err(_) => AppError::Internal("Invalid stored entry JSON".into()).into_response(),
        },
        Err(error) => error.into_response(),
    }
}
