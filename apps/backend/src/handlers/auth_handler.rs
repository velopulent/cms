use axum::{
    Json,
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::models::user::{ChangePasswordRequest, CreateUser, LoginRequest, UpdateSelfProfile};
use crate::services::Services;

#[instrument(skip(services, payload))]
pub async fn register(Extension(services): Extension<Services>, Json(payload): Json<CreateUser>) -> Response {
    let user = match services
        .auth
        .register(&payload.name, &payload.email, &payload.password)
        .await
    {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    let (token, csrf_token) = match services.auth.login(&payload.email, &payload.password).await {
        Ok((_, token, csrf)) => (token, csrf),
        Err(e) => return e.into_response(),
    };

    services.auth.build_register_response(user, &token, &csrf_token)
}

#[instrument(skip(services, payload))]
pub async fn login(Extension(services): Extension<Services>, Json(payload): Json<LoginRequest>) -> Response {
    let (user, token, csrf_token) = match services.auth.login(&payload.email, &payload.password).await {
        Ok(result) => result,
        Err(e) => return e.into_response(),
    };

    services.auth.build_auth_cookies_response(user, &token, &csrf_token)
}

pub async fn logout(services: Extension<Services>, auth: crate::middleware::auth::AuthContext) -> Response {
    if let crate::middleware::auth::Actor::User(user) = &auth.actor
        && let Err(error) = services.auth.logout(&user.session_id, &user.user_id).await
    {
        return error.into_response();
    }
    services.auth.build_logout_response()
}

#[instrument(skip(services))]
pub async fn me(services: Extension<Services>, auth: crate::middleware::auth::AuthContext) -> Response {
    let user_id = match &auth.actor {
        crate::middleware::auth::Actor::User(u) => &u.user_id,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Session authentication required"})),
            )
                .into_response();
        }
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

pub async fn list_sessions(
    Extension(services): Extension<Services>,
    auth: crate::middleware::auth::AuthContext,
) -> Response {
    let crate::middleware::auth::Actor::User(user) = &auth.actor else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Session authentication required"})),
        )
            .into_response();
    };
    match services.auth.list_sessions(&user.user_id, &user.session_id).await {
        Ok(sessions) => (StatusCode::OK, Json(sessions)).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn revoke_all_sessions(
    Extension(services): Extension<Services>,
    auth: crate::middleware::auth::AuthContext,
) -> Response {
    let crate::middleware::auth::Actor::User(user) = &auth.actor else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Session authentication required"})),
        )
            .into_response();
    };
    match services.auth.revoke_all_sessions(&user.user_id).await {
        Ok(_) => services.auth.build_logout_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn change_password(
    Extension(services): Extension<Services>,
    auth: crate::middleware::auth::AuthContext,
    Json(payload): Json<ChangePasswordRequest>,
) -> Response {
    let crate::middleware::auth::Actor::User(user) = &auth.actor else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Session authentication required"})),
        )
            .into_response();
    };
    match services
        .auth
        .change_password(&user.user_id, &payload.current_password, &payload.new_password)
        .await
    {
        Ok(()) => services.auth.build_logout_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn update_me(
    Extension(services): Extension<Services>,
    auth: crate::middleware::auth::AuthContext,
    Json(payload): Json<UpdateSelfProfile>,
) -> Response {
    let crate::middleware::auth::Actor::User(user) = &auth.actor else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Session authentication required"})),
        )
            .into_response();
    };
    match services.auth.update_self_name(&user.user_id, &payload.name).await {
        Ok(user) => (StatusCode::OK, Json(user)).into_response(),
        Err(error) => error.into_response(),
    }
}
