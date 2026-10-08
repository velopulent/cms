use axum::{
    Json,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::config::Config;
use crate::middleware::auth::{Actor, AuthContext, AuthMethod, verify_access_token};
use crate::repository::Repository;

pub async fn api_auth_middleware(mut request: Request, next: Next) -> Response {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(crate::middleware::auth::parse_bearer_header)
        .map(|v| v.trim().to_string());

    let token = match auth_header {
        Some(t) if t.starts_with("vcms_site_") || t.starts_with("vcms_pat_") => t,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized", "message": "Valid API key required"})),
            )
                .into_response();
        }
    };

    let repository = match request.extensions().get::<Repository>() {
        Some(r) => r.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal_error", "message": "Repository not available"})),
            )
                .into_response();
        }
    };

    let config = match request.extensions().get::<Config>() {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal_error", "message": "Config not available"})),
            )
                .into_response();
        }
    };

    let actor = match verify_access_token(&token, &repository, &config.token_index_key).await {
        Ok(actor) => actor,
        Err((status, err)) => return (status, err).into_response(),
    };

    let auth_method = match &actor {
        Actor::ApiKey(_) => AuthMethod::ApiKey,
        Actor::PersonalToken(_) => AuthMethod::PersonalToken,
        Actor::User(_) => AuthMethod::Session,
    };
    request.extensions_mut().insert(AuthContext {
        actor: actor.clone(),
        auth_method,
    });
    request.extensions_mut().insert(actor);
    next.run(request).await
}
