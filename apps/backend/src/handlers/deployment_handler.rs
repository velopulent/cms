use crate::{
    middleware::auth::{RequestContext, require_site_action},
    models::{authorization::Action, deployment::CreateDeploymentTrigger},
    repository::Repository,
    services::Services,
};
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub async fn list(
    ctx: RequestContext,
    Path(site_id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsRead).await {
        return v.into_response();
    }
    match services.deployment.list(&site_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":e}))).into_response(),
    }
}
pub async fn create(
    ctx: RequestContext,
    Path(site_id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(value): Json<CreateDeploymentTrigger>,
) -> Response {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsWrite).await {
        return v.into_response();
    }
    let user = ctx.auth.actor.user_id().unwrap_or("system");
    match services.deployment.create(&site_id, user, value).await {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error":e}))).into_response(),
    }
}
pub async fn update(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(value): Json<CreateDeploymentTrigger>,
) -> Response {
    if let Err(response) = require_site_action(&ctx, &repo, Action::DeploymentsWrite).await {
        return response.into_response();
    }
    match services.deployment.update(&site_id, &trigger_id, value).await {
        Ok(trigger) => Json(trigger).into_response(),
        Err(error) if error == "trigger_not_found" => StatusCode::NOT_FOUND.into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error": error}))).into_response(),
    }
}
pub async fn trigger(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsTrigger).await {
        return v.into_response();
    }
    let user = ctx.auth.actor.user_id().unwrap_or("site-key");
    match services.deployment.trigger(&site_id,&trigger_id,user).await{Ok(v)=>(StatusCode::ACCEPTED,Json(v)).into_response(),Err(e)if e.starts_with("deployment_cooldown:")=>(StatusCode::TOO_MANY_REQUESTS,Json(json!({"error":"deployment_cooldown","retry_after_seconds":e.split(':').nth(1).and_then(|v|v.parse::<i64>().ok())}))).into_response(),Err(e)if e=="deployment_daily_quota"=>(StatusCode::TOO_MANY_REQUESTS,Json(json!({"error":e}))).into_response(),Err(e)=>(StatusCode::CONFLICT,Json(json!({"error":e}))).into_response()}
}
pub async fn history(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsRead).await {
        return v.into_response();
    }
    if !services
        .deployment
        .list(&site_id)
        .await
        .unwrap_or_default()
        .iter()
        .any(|trigger| trigger.id == trigger_id)
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    match services.deployment.history(&trigger_id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":e}))).into_response(),
    }
}
pub async fn delete(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err(value) = require_site_action(&ctx, &repo, Action::DeploymentsWrite).await {
        return value.into_response();
    }
    match services.deployment.delete(&site_id, &trigger_id).await {
        Ok(0) => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":error}))).into_response(),
    }
}
