use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tracing::instrument;

#[derive(Deserialize)]
pub struct EntryId {
    id: String,
}

#[derive(Deserialize)]
pub struct RevisionId {
    id: String,
    number: i64,
}

use crate::middleware::auth::{Actor, RequestContext, Scope, require_site_scope};
use crate::models::entry::{CreateEntry, Entry, EntryRevisionResponse, RevisionsListResponse, UpdateEntry};
use crate::repository::Repository;
use crate::repository::traits::ListEntriesParams;
use crate::services::Services;
use crate::storage::{StorageProvider, StorageRegistry};
use crate::utils::diff::compute_diff_for_revision;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListParams {
    pub r#type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesRead, "viewer").await {
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesRead, "viewer").await {
        return (status, err).into_response();
    }

    let published_only = matches!(ctx.auth.actor, Actor::ApiKey(_));

    match services.entry.get_entry(&id, &ctx.site_id, published_only).await {
        Ok(Some(item)) => {
            let storage_provider = services
                .file
                .get_storage_provider(&ctx.site_id)
                .await
                .unwrap_or_else(|_| "filesystem".into());
            let storage = match get_storage_for_site(&storage_provider, &storage_registry) {
                Ok(s) => s,
                Err(resp) => return resp,
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesWrite, "editor").await {
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

#[utoipa::path(
    put,
    path = "/api/v1/entries/{id}",
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesWrite, "editor").await {
        return (status, err).into_response();
    }

    let created_by = ctx.auth.actor.user_id();
    match services
        .entry
        .update_entry(
            &id,
            &ctx.site_id,
            payload.data.as_ref(),
            payload.slug.as_deref(),
            payload.status.as_deref(),
            created_by,
            payload.change_summary.as_deref(),
        )
        .await
    {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/entries/{id}",
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesWrite, "editor").await {
        return (status, err).into_response();
    }

    match services.entry.delete_entry(&id, &ctx.site_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/entries/{id}/publish",
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesWrite, "editor").await {
        return (status, err).into_response();
    }

    match services.entry.publish_entry(&id, &ctx.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/entries/{id}/unpublish",
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesWrite, "editor").await {
        return (status, err).into_response();
    }

    match services.entry.unpublish_entry(&id, &ctx.site_id).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/entries/{id}/revisions",
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesRead, "viewer").await {
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
    path = "/api/v1/entries/{id}/revisions/{number}",
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesRead, "viewer").await {
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

            if query.diff.unwrap_or(false) && number > 1
                && let Ok(Some(prev)) = services.entry.get_revision(&id, &ctx.site_id, number - 1).await
                    && let Some(diff) = compute_diff_for_revision(&revision, Some(&prev)) {
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
    path = "/api/v1/entries/{id}/revisions/{number}/restore",
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
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::EntriesWrite, "editor").await {
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
