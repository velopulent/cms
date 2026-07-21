use axum::{
    Json,
    extract::{FromRequestParts, Path, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use serde::Deserialize;

use crate::middleware::auth::{Actor, AuthContext, AuthMethod, RequestContext};
use crate::models::authorization::Action;
use crate::repository::Repository;

#[derive(Deserialize)]
pub struct SiteIdParam {
    site_id: String,
}

pub async fn api_site_resolver(mut request: Request, next: Next) -> Response {
    let actor = match request.extensions().get::<Actor>() {
        Some(actor) => actor.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized", "message": "Authentication required"})),
            )
                .into_response();
        }
    };

    let site_id = match &actor {
        Actor::ApiKey(k) => k.site_id.clone(),
        Actor::PersonalToken(_) => match request.headers().get("x-vcms-site").and_then(|v|v.to_str().ok()) { Some(v) if !v.is_empty()=>v.to_string(), _=>return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"missing_site_context","message":"X-VCMS-Site is required for personal tokens"}))).into_response() },
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "forbidden",
                    "message": "Public API requires API key authentication"
                })),
            )
                .into_response();
        }
    };

    if let Actor::PersonalToken(token) = &actor {
        let repository = match request.extensions().get::<Repository>() {
            Some(value) => value.clone(),
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error":"internal_error"})),
                )
                    .into_response();
            }
        };
        if let Err((status, error)) =
            crate::middleware::auth::check_site_action_repo(&repository, &token.user_id, &site_id, Action::SiteRead)
                .await
        {
            return (status, error).into_response();
        }
    }

    let auth_method = if matches!(actor, Actor::PersonalToken(_)) {
        AuthMethod::PersonalToken
    } else {
        AuthMethod::ApiKey
    };
    let auth = AuthContext { actor, auth_method };

    let ctx = RequestContext { site_id, auth };
    request.extensions_mut().insert(ctx);
    next.run(request).await
}

pub async fn dashboard_site_resolver(request: Request, next: Next) -> Response {
    let actor = match request.extensions().get::<Actor>() {
        Some(actor) => actor.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized", "message": "Authentication required"})),
            )
                .into_response();
        }
    };

    let (mut parts, body) = request.into_parts();

    let site_id: String = match Path::<SiteIdParam>::from_request_parts(&mut parts, &()).await {
        Ok(params) => params.site_id.clone(),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "bad_request", "message": "Missing site id in URL"})),
            )
                .into_response();
        }
    };

    if let Actor::User(user) = &actor {
        let repository = match parts.extensions.get::<Repository>() {
            Some(r) => r.clone(),
            None => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "internal_error", "message": "Repository not available"})),
                )
                    .into_response();
            }
        };

        if let Err((status, err)) =
            crate::middleware::auth::check_site_action_repo(&repository, &user.user_id, &site_id, Action::SiteRead)
                .await
        {
            return (status, err).into_response();
        }
    }

    let auth = AuthContext {
        actor,
        auth_method: AuthMethod::Session,
    };

    let ctx = RequestContext { site_id, auth };
    parts.extensions.insert(ctx);

    let request = Request::from_parts(parts, body);
    next.run(request).await
}
