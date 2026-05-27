use axum::{
    Json,
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::models::user::{CreateUser, LoginRequest};
use crate::services::Services;

#[instrument(skip(services, payload))]
pub async fn register(Extension(services): Extension<Services>, Json(payload): Json<CreateUser>) -> Response {
    let user = match services
        .auth
        .register(&payload.username, &payload.email, &payload.password)
        .await
    {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    let token = match services.auth.login(&payload.username, &payload.password).await {
        Ok((_, t)) => t,
        Err(e) => return e.into_response(),
    };

    services.auth.build_register_response(user, &token)
}

#[instrument(skip(services, payload))]
pub async fn login(Extension(services): Extension<Services>, Json(payload): Json<LoginRequest>) -> Response {
    let (user, token) = match services.auth.login(&payload.username, &payload.password).await {
        Ok((u, t)) => (u, t),
        Err(e) => return e.into_response(),
    };

    services.auth.build_auth_cookies_response(user, &token)
}

pub async fn logout(services: Extension<Services>) -> Response {
    services.auth.build_logout_response()
}

#[instrument(skip(services))]
pub async fn me(
    services: Extension<Services>,
    auth: crate::middleware::auth::AuthContext,
) -> Response {
    let user_id = match &auth.actor {
        crate::middleware::auth::Actor::User(u) => &u.user_id,
        _ => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Session authentication required"}))).into_response(),
    };
    match services.auth.get_user(user_id).await {
        Ok(Some(user)) => {
            use axum::{Json, http::StatusCode};
            (StatusCode::OK, Json(user)).into_response()
        }
        Ok(None) => {
            use axum::{Json, http::StatusCode};
            let body = serde_json::json!({"error": "User not found"});
            (StatusCode::NOT_FOUND, Json(body)).into_response()
        }
        Err(e) => e.into_response(),
    }
}
