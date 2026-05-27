use axum::{
    Json,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::config::Config;
use crate::middleware::auth::verify_access_token;
use crate::repository::Repository;

pub async fn api_auth_middleware(mut request: Request, next: Next) -> Response {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.trim().to_string());

    let token = match auth_header {
        Some(t) if t.starts_with("cms_site_") => t,
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

    let actor = match verify_access_token(&token, &repository, &config.hmac_secret).await {
        Ok(actor) => actor,
        Err((status, err)) => return (status, err).into_response(),
    };

    request.extensions_mut().insert(actor);
    next.run(request).await
}
