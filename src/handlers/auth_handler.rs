use axum::{
    Json,
    extract::Extension,
    http::{HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::Cookie;
use bcrypt::{DEFAULT_COST, hash, verify};
use regex::Regex;
use serde_json::json;
use time::Duration;
use tracing::instrument;
use uuid::Uuid;

use crate::config::Config;
use crate::middleware::auth::{AuthenticatedUser, create_token};
use crate::models::user::{AuthResponse, CreateUser, LoginRequest, UserPublic};
use crate::repository::error::RepositoryError;
use crate::repository::Repository;

static EMAIL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap());

#[instrument(skip(repository, config, payload), fields(username = %payload.username))]
pub async fn register(
    Extension(repository): Extension<Repository>,
    Extension(config): Extension<Config>,
    Json(payload): Json<CreateUser>,
) -> Response {
    let username = payload.username.trim();
    let password = payload.password.trim();
    let email = payload.email.trim();

    if username.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Username is required"})),
        )
            .into_response();
    }

    if username.len() < 3 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Username must be at least 3 characters"})),
        )
            .into_response();
    }

    if password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Password is required"})),
        )
            .into_response();
    }

    if password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Password must be at least 8 characters"})),
        )
            .into_response();
    }

    if !EMAIL_RE.is_match(email) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email address"})),
        )
            .into_response();
    }

    let password_hash = match hash(password, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Hash error: {}", e)})),
            )
                .into_response();
        }
    };

    let id = Uuid::now_v7().to_string();

    match repository.user.create(&id, username, email, &password_hash).await {
        Ok(_) => {
            let token = match create_token(id.clone(), &config.jwt_secret) {
                Ok(t) => t,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Token error: {}", e)})),
                    )
                        .into_response();
                }
            };

            let user = UserPublic {
                id,
                username: username.to_string(),
                email: email.to_string(),
            };

            let mut response = build_auth_cookies_response(user, &token, &config);
            *response.status_mut() = StatusCode::CREATED;
            response
        }
        Err(RepositoryError::UniqueViolation(_)) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Username or email already exists"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[instrument(skip(repository, config, payload), fields(username = %payload.username))]
pub async fn login(
    Extension(repository): Extension<Repository>,
    Extension(config): Extension<Config>,
    Json(payload): Json<LoginRequest>,
) -> Response {
    let user = match repository.user.find_by_username(&payload.username).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid username or password"})),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    match verify(&payload.password, &user.password_hash) {
        Ok(true) => {}
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid username or password"})),
            )
                .into_response();
        }
    };

    let token = match create_token(user.id.clone(), &config.jwt_secret) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Token error: {}", e)})),
            )
                .into_response();
        }
    };

    build_auth_cookies_response(
        UserPublic {
            id: user.id,
            username: user.username,
            email: user.email,
        },
        &token,
        &config,
    )
}

fn build_auth_cookies_response(
    user: UserPublic,
    jwt: &str,
    config: &Config,
) -> Response {
    let token_cookie = Cookie::build(("token", jwt.to_string()))
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(axum_extra::extract::cookie::SameSite::Strict)
        .path("/")
        .max_age(Duration::hours(24))
        .build();

    let csrf_token = Uuid::now_v7().to_string();
    let csrf_cookie = Cookie::build(("csrf", csrf_token.clone()))
        .http_only(false)
        .secure(config.cookie_secure)
        .same_site(axum_extra::extract::cookie::SameSite::Strict)
        .path("/")
        .max_age(Duration::hours(24))
        .build();

    let mut response = (
        StatusCode::OK,
        Json(AuthResponse { user }),
    )
        .into_response();

    if let Ok(val) = HeaderValue::from_str(&token_cookie.to_string()) {
        response.headers_mut().insert(SET_COOKIE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&csrf_cookie.to_string()) {
        response.headers_mut().append(SET_COOKIE, val);
    }

    response
}

#[instrument]
pub async fn logout() -> Response {
    let clear_token = Cookie::build(("token", ""))
        .http_only(true)
        .path("/")
        .max_age(Duration::ZERO)
        .build();

    let clear_csrf = Cookie::build(("csrf", ""))
        .http_only(false)
        .path("/")
        .max_age(Duration::ZERO)
        .build();

    let mut response = (
        StatusCode::OK,
        Json(json!({"message": "Logged out"})),
    )
        .into_response();

    if let Ok(val) = HeaderValue::from_str(&clear_token.to_string()) {
        response.headers_mut().insert(SET_COOKIE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&clear_csrf.to_string()) {
        response.headers_mut().append(SET_COOKIE, val);
    }

    response
}

#[instrument(skip(repository))]
pub async fn me(auth: AuthenticatedUser, Extension(repository): Extension<Repository>) -> Response {
    match repository.user.find_by_id(&auth.user_id).await {
        Ok(Some(u)) => (
            StatusCode::OK,
            Json(UserPublic {
                id: u.id,
                username: u.username,
                email: u.email,
            }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}