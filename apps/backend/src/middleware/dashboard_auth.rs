use axum::{
    Json,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::config::Config;
use crate::middleware::auth::{Actor, compute_key_hmac, verify_csrf, verify_session};

pub async fn dashboard_auth_middleware(mut request: Request, next: Next) -> Response {
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

    let token = request
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix("token=").map(|val| val.to_string())
            })
        });

    let token = match token {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized", "message": "Authentication required"})),
            )
                .into_response();
        }
    };

    let repository = match request.extensions().get::<crate::repository::Repository>() {
        Some(repository) => repository.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal_error", "message": "Repository not available"})),
            )
                .into_response();
        }
    };
    let user = match verify_session(&token, &repository, &config.hmac_secret).await {
        Ok(user) => user,
        Err((status, error)) => return (status, error).into_response(),
    };

    if matches!(request.method().as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
        let session = match repository
            .session
            .find_active_by_hash(&compute_key_hmac(&token, &config.hmac_secret))
            .await
        {
            Ok(Some(session)) => session,
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "unauthorized", "message": "Invalid session"})),
                )
                    .into_response();
            }
        };
        let (parts, body) = request.into_parts();
        if let Err((status, msg)) = verify_csrf(&parts, &config.hmac_secret, &session.csrf_token_hash) {
            return (status, Json(serde_json::json!({"error": "csrf_error", "message": msg}))).into_response();
        }
        request = Request::from_parts(parts, body);
    }
    let actor = Actor::User(user);

    request.extensions_mut().insert(actor);
    next.run(request).await
}
