use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;

#[derive(Deserialize)]
pub struct WebhookId {
    webhook_id: String,
}

use crate::middleware::auth::{RequestContext, Scope, require_site_scope};
use crate::models::webhook::{CreateWebhook, UpdateWebhook};
use crate::repository::Repository;
use crate::services::Services;

#[derive(Deserialize, Debug, utoipa::IntoParams)]
pub struct ListDeliveriesParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/webhooks",
    responses(
        (status = 200, description = "List webhooks for a site"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn list_webhooks(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::WebhooksRead, "viewer").await {
        return (status, err).into_response();
    }

    match services.webhook.list_webhooks(&ctx.site_id).await {
        Ok(webhooks) => {
            let masked: Vec<serde_json::Value> = webhooks
                .into_iter()
                .map(|w| -> serde_json::Value {
                    serde_json::json!({
                        "id": w.id,
                        "site_id": w.site_id,
                        "label": w.label,
                        "url": w.url,
                        "created_by": w.created_by,
                        "created_at": w.created_at,
                        "updated_at": w.updated_at,
                    })
                })
                .collect();
            (StatusCode::OK, Json(masked)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks",
    request_body = CreateWebhook,
    responses(
        (status = 201, description = "Webhook created"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, ctx, payload))]
pub async fn create_webhook(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateWebhook>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::WebhooksWrite, "admin").await {
        return (status, err).into_response();
    }

    let created_by = ctx.auth.actor.user_id();
    let webhook = services
        .webhook
        .create_webhook(&ctx.site_id, &payload.label, &payload.url, &payload.headers, created_by)
        .await;
    match webhook {
        Ok(webhook) => (StatusCode::CREATED, Json(webhook)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/webhooks/{webhook_id}",
    params(("webhook_id" = String, Path, description = "Webhook ID")),
    responses(
        (status = 200, description = "Webhook details"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Webhook not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn get_webhook(
    ctx: RequestContext,
    Path(WebhookId { webhook_id }): Path<WebhookId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::WebhooksRead, "viewer").await {
        return (status, err).into_response();
    }

    match services.webhook.get_webhook(&webhook_id, &ctx.site_id).await {
        Ok(Some(webhook)) => (StatusCode::OK, Json(webhook)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Webhook not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/webhooks/{webhook_id}",
    params(("webhook_id" = String, Path, description = "Webhook ID")),
    request_body = UpdateWebhook,
    responses(
        (status = 200, description = "Webhook updated"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, ctx, payload))]
pub async fn update_webhook(
    ctx: RequestContext,
    Path(WebhookId { webhook_id }): Path<WebhookId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateWebhook>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::WebhooksWrite, "admin").await {
        return (status, err).into_response();
    }

    match services
        .webhook
        .update_webhook(&webhook_id, &ctx.site_id, payload.label.as_deref(), payload.url.as_deref(), payload.headers.as_ref())
        .await
    {
        Ok(webhook) => (StatusCode::OK, Json(webhook)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/webhooks/{webhook_id}",
    params(("webhook_id" = String, Path, description = "Webhook ID")),
    responses(
        (status = 204, description = "Webhook deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn delete_webhook(
    ctx: RequestContext,
    Path(WebhookId { webhook_id }): Path<WebhookId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::WebhooksWrite, "admin").await {
        return (status, err).into_response();
    }

    match services.webhook.delete_webhook(&webhook_id, &ctx.site_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Webhook not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/{webhook_id}/trigger",
    params(("webhook_id" = String, Path, description = "Webhook ID")),
    responses(
        (status = 200, description = "Webhook triggered"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Webhook not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn trigger_webhook(
    ctx: RequestContext,
    Path(WebhookId { webhook_id }): Path<WebhookId>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::WebhooksWrite, "editor").await {
        return (status, err).into_response();
    }

    let triggered_by = ctx.auth.actor.user_id();
    match services
        .webhook
        .trigger_webhook(&webhook_id, &ctx.site_id, triggered_by)
        .await
    {
        Ok(delivery) => (StatusCode::OK, Json(delivery)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/webhooks/{webhook_id}/deliveries",
    params(
        ("webhook_id" = String, Path, description = "Webhook ID"),
        ListDeliveriesParams,
    ),
    responses(
        (status = 200, description = "List of webhook deliveries"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Webhook not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn list_deliveries(
    ctx: RequestContext,
    Path(WebhookId { webhook_id }): Path<WebhookId>,
    Query(params): Query<ListDeliveriesParams>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_scope(&ctx, &repository, &Scope::WebhooksRead, "viewer").await {
        return (status, err).into_response();
    }

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);

    match services.webhook.get_webhook(&webhook_id, &ctx.site_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Webhook not found"}))).into_response(),
        Err(e) => return e.into_response(),
    }

    match services
        .webhook
        .list_deliveries(&webhook_id, &ctx.site_id, page, per_page)
        .await
    {
        Ok((deliveries, total)) => {
            (StatusCode::OK, Json(serde_json::json!({
                "items": deliveries,
                "total": total,
                "page": page,
                "per_page": per_page,
            })))
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}
