use crate::{
    error::AppError,
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
) -> Result<Response, AppError> {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsRead).await {
        return Ok(v.into_response());
    }
    match services.deployment.list(&site_id).await {
        Ok(v) => Ok(Json(v).into_response()),
        Err(e) => Err(AppError::Internal(e)),
    }
}
pub async fn create(
    ctx: RequestContext,
    Path(site_id): Path<String>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(value): Json<CreateDeploymentTrigger>,
) -> Result<Response, AppError> {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsWrite).await {
        return Ok(v.into_response());
    }
    let user = ctx.auth.actor.user_id();
    match services.deployment.create(&site_id, user, value).await {
        Ok(v) => Ok((StatusCode::CREATED, Json(v)).into_response()),
        Err(e) => Err(AppError::BadRequest(e)),
    }
}
pub async fn update(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(value): Json<CreateDeploymentTrigger>,
) -> Result<Response, AppError> {
    if let Err(response) = require_site_action(&ctx, &repo, Action::DeploymentsWrite).await {
        return Ok(response.into_response());
    }
    match services.deployment.update(&site_id, &trigger_id, value).await {
        Ok(trigger) => Ok(Json(trigger).into_response()),
        Err(error) if error == "trigger_not_found" => Err(AppError::NotFound(error)),
        Err(error) => Err(AppError::BadRequest(error)),
    }
}
pub async fn trigger(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Result<Response, AppError> {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsTrigger).await {
        return Ok(v.into_response());
    }
    let user = ctx.auth.actor.user_id();
    match services.deployment.trigger(&site_id, &trigger_id, user).await {
        Ok(v) => Ok((StatusCode::ACCEPTED, Json(v)).into_response()),
        Err(e) if e.starts_with("deployment_cooldown:") => Ok((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": "deployment_cooldown",
                "retry_after_seconds": e.split(':').nth(1).and_then(|v| v.parse::<i64>().ok())
            })),
        )
            .into_response()),
        Err(e) if e == "deployment_daily_quota" => Err(AppError::TooManyRequests(e)),
        Err(e) if e == "deployment_in_progress" => Err(AppError::Conflict(e)),
        Err(e) if e == "trigger_not_found" => Err(AppError::NotFound(e)),
        Err(e) => Err(AppError::BadRequest(e)),
    }
}
pub async fn history(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Result<Response, AppError> {
    if let Err(v) = require_site_action(&ctx, &repo, Action::DeploymentsRead).await {
        return Ok(v.into_response());
    }
    match services.deployment.get(&site_id, &trigger_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return Err(AppError::NotFound("trigger_not_found".into())),
        Err(error) => return Err(AppError::Internal(error)),
    }
    match services.deployment.history(&trigger_id).await {
        Ok(v) => Ok(Json(v).into_response()),
        Err(e) => Err(AppError::Internal(e)),
    }
}
pub async fn delete(
    ctx: RequestContext,
    Path((site_id, trigger_id)): Path<(String, String)>,
    Extension(repo): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Result<Response, AppError> {
    if let Err(value) = require_site_action(&ctx, &repo, Action::DeploymentsWrite).await {
        return Ok(value.into_response());
    }
    match services.deployment.delete(&site_id, &trigger_id).await {
        Ok(0) => Err(AppError::NotFound("trigger_not_found".into())),
        Ok(_) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(error) => Err(AppError::Internal(error)),
    }
}
