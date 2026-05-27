use axum::{
    Json,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::config::Config;
use crate::middleware::auth::{Actor, UserActor, verify_csrf, verify_token};

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
                if let Some(val) = c.strip_prefix("token=") {
                    Some(val.to_string())
                } else {
                    None
                }
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

    if matches!(request.method().as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
        let (parts, _body) = request.into_parts();
        if let Err((status, msg)) = verify_csrf(&parts, &config) {
            return (status, Json(serde_json::json!({"error": "csrf_error", "message": msg})))
                .into_response();
        }
        request = Request::from_parts(parts, _body);
    }

    let claims = match verify_token(&token, &config.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized", "message": "Invalid or expired session"})),
            )
                .into_response();
        }
    };

    let actor = Actor::User(UserActor {
        user_id: claims.sub,
    });

    request.extensions_mut().insert(actor);
    next.run(request).await
}
