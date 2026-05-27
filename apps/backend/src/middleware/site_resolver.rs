use axum::{
    Json,
    extract::{FromRequestParts, Path, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use serde::Deserialize;

use crate::middleware::auth::{
    Actor, AuthContext, AuthMethod, RequestContext, ScopeSet, check_site_access_repo,
};
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

    let scopes = match &actor {
        Actor::ApiKey(k) => ScopeSet::from_permission(&k.permission),
        _ => ScopeSet::all(),
    };

    let auth = AuthContext {
        actor,
        auth_method: AuthMethod::ApiKey,
        scopes,
    };

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
            check_site_access_repo(&repository, &user.user_id, &site_id, "viewer").await
        {
            return (status, err).into_response();
        }
    }

    let scopes = ScopeSet::all();
    let auth = AuthContext {
        actor,
        auth_method: AuthMethod::JwtSession,
        scopes,
    };

    let ctx = RequestContext { site_id, auth };
    parts.extensions.insert(ctx);

    let request = Request::from_parts(parts, body);
    next.run(request).await
}
