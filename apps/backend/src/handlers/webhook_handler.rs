use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{
    Principal, SCOPE_WEBHOOKS_READ, SCOPE_WEBHOOKS_TRIGGER, SCOPE_WEBHOOKS_WRITE, require_admin_scope,
};
use crate::models::webhook::{CreateWebhook, UpdateWebhook, WebhookDelivery};
use crate::repository::Repository;
use crate::services::Services;

#[derive(Deserialize, Debug)]
pub struct ListDeliveriesParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/webhooks",
    params(("site_id" = String, Path, description = "Site id")),
    responses(
        (status = 200, description = "List webhooks for a site"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, principal))]
pub async fn list_webhooks(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_WEBHOOKS_READ).await
    {
        return (status, err).into_response();
    }

    match services.webhook.list_webhooks(&site_id).await {
        Ok(webhooks) => {
            let masked: Vec<serde_json::Value> = webhooks
                .into_iter()
                .map(|w| {
                    json!({
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
            (StatusCode::OK, Json(json!(masked))).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/webhooks",
    params(("site_id" = String, Path, description = "Site id")),
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
#[instrument(skip(repository, services, principal, payload))]
pub async fn create_webhook(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateWebhook>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_WEBHOOKS_WRITE).await
    {
        return (status, err).into_response();
    }

    let created_by = principal.user_id().unwrap_or("system");

    match services
        .webhook
        .create_webhook(&site_id, &payload.label, &payload.url, &payload.headers, created_by)
        .await
    {
        Ok(webhook) => (StatusCode::CREATED, Json(json!({
            "id": webhook.id,
            "site_id": webhook.site_id,
            "label": webhook.label,
            "url": webhook.url,
            "created_by": webhook.created_by,
            "created_at": webhook.created_at,
            "updated_at": webhook.updated_at,
        })))
        .into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/webhooks/{webhook_id}",
    params(
        ("site_id" = String, Path, description = "Site id"),
        ("webhook_id" = String, Path, description = "Webhook id")
    ),
    responses(
        (status = 200, description = "Webhook details"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Webhook not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, principal))]
pub async fn get_webhook(
    principal: Principal,
    Path((site_id, webhook_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_WEBHOOKS_READ).await
    {
        return (status, err).into_response();
    }

    match services.webhook.get_webhook(&webhook_id, &site_id).await {
        Ok(Some(webhook)) => {
            let headers = services.webhook.decrypt_webhook_headers(&webhook);
            (StatusCode::OK, Json(json!({
                "id": webhook.id,
                "site_id": webhook.site_id,
                "label": webhook.label,
                "url": webhook.url,
                "headers": headers,
                "created_by": webhook.created_by,
                "created_at": webhook.created_at,
                "updated_at": webhook.updated_at,
            })))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Webhook not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}/webhooks/{webhook_id}",
    params(
        ("site_id" = String, Path, description = "Site id"),
        ("webhook_id" = String, Path, description = "Webhook id")
    ),
    request_body = UpdateWebhook,
    responses(
        (status = 200, description = "Webhook updated"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Webhook not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, principal, payload))]
pub async fn update_webhook(
    principal: Principal,
    Path((site_id, webhook_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateWebhook>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_WEBHOOKS_WRITE).await
    {
        return (status, err).into_response();
    }

    match services
        .webhook
        .update_webhook(
            &webhook_id,
            &site_id,
            payload.label.as_deref(),
            payload.url.as_deref(),
            payload.headers.as_ref(),
        )
        .await
    {
        Ok(webhook) => (StatusCode::OK, Json(json!({
            "id": webhook.id,
            "site_id": webhook.site_id,
            "label": webhook.label,
            "url": webhook.url,
            "created_by": webhook.created_by,
            "created_at": webhook.created_at,
            "updated_at": webhook.updated_at,
        })))
        .into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/webhooks/{webhook_id}",
    params(
        ("site_id" = String, Path, description = "Site id"),
        ("webhook_id" = String, Path, description = "Webhook id")
    ),
    responses(
        (status = 204, description = "Webhook deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, principal))]
pub async fn delete_webhook(
    principal: Principal,
    Path((site_id, webhook_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_WEBHOOKS_WRITE).await
    {
        return (status, err).into_response();
    }

    match services.webhook.delete_webhook(&webhook_id, &site_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Webhook not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/webhooks/{webhook_id}/trigger",
    params(
        ("site_id" = String, Path, description = "Site id"),
        ("webhook_id" = String, Path, description = "Webhook id")
    ),
    responses(
        (status = 200, description = "Webhook triggered"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Webhook not found"),
        (status = 502, description = "Webhook delivery failed"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, principal))]
pub async fn trigger_webhook(
    principal: Principal,
    Path((site_id, webhook_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) =
        require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_WEBHOOKS_TRIGGER).await
    {
        return (status, err).into_response();
    }

    let triggered_by = principal.user_id().unwrap_or("system");

    match services.webhook.trigger_webhook(&webhook_id, &site_id, triggered_by).await {
        Ok(delivery) => {
            let status = if delivery.status == "success" {
                StatusCode::OK
            } else {
                StatusCode::BAD_GATEWAY
            };
            (status, Json(json!({
                "id": delivery.id,
                "webhook_id": delivery.webhook_id,
                "status": delivery.status,
                "status_code": delivery.status_code,
                "response_body": delivery.response_body,
                "duration_ms": delivery.duration_ms,
                "triggered_by": delivery.triggered_by,
                "triggered_at": delivery.triggered_at,
            })))
            .into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/webhooks/{webhook_id}/deliveries",
    params(
        ("site_id" = String, Path, description = "Site id"),
        ("webhook_id" = String, Path, description = "Webhook id")
    ),
    responses(
        (status = 200, description = "List webhook deliveries"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Webhook not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "webhooks"
)]
#[instrument(skip(repository, services, principal))]
pub async fn list_deliveries(
    principal: Principal,
    Path((site_id, webhook_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Query(params): Query<ListDeliveriesParams>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_WEBHOOKS_READ).await
    {
        return (status, err).into_response();
    }

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);

    match services.webhook.list_deliveries(&webhook_id, &site_id, page, per_page).await {
        Ok((deliveries, total)) => {
            let items: Vec<serde_json::Value> = deliveries
                .into_iter()
                .map(|d: WebhookDelivery| {
                    json!({
                        "id": d.id,
                        "webhook_id": d.webhook_id,
                        "status": d.status,
                        "status_code": d.status_code,
                        "response_body": d.response_body,
                        "duration_ms": d.duration_ms,
                        "triggered_by": d.triggered_by,
                        "triggered_at": d.triggered_at,
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({
                "items": items,
                "total": total,
                "page": page,
                "per_page": per_page,
            })))
            .into_response()
        }
        Err(e) => e.into_response(),
    }
}