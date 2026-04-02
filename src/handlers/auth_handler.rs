use axum::{
    Json,
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bcrypt::{DEFAULT_COST, hash, verify};
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::config::Config;
use crate::middleware::auth::{AuthenticatedUser, create_token};
use crate::models::user::{AuthResponse, CreateUser, LoginRequest, UserPublic};
use crate::repository::user as user_repo;

#[utoipa::path(
    post,
    path = "/api/auth/register",
    request_body = CreateUser,
    responses(
        (status = 201, description = "User registered successfully", body = AuthResponse),
        (status = 409, description = "Username or email already exists"),
    ),
    tag = "auth"
)]
pub async fn register(
    Extension(pool): Extension<SqlitePool>,
    Extension(config): Extension<Config>,
    Json(payload): Json<CreateUser>,
) -> Response {
    if payload.username.trim().is_empty() || payload.password.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Username and password are required"})),
        )
            .into_response();
    }

    let password_hash = match hash(&payload.password, DEFAULT_COST) {
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

    match user_repo::create(&pool, &id, &payload.username, &payload.email, &password_hash).await {
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

            (
                StatusCode::CREATED,
                Json(AuthResponse {
                    token,
                    user: UserPublic {
                        id,
                        username: payload.username,
                        email: payload.email,
                    },
                }),
            )
                .into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
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

#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
    ),
    tag = "auth"
)]
pub async fn login(
    Extension(pool): Extension<SqlitePool>,
    Extension(config): Extension<Config>,
    Json(payload): Json<LoginRequest>,
) -> Response {
    let user = match user_repo::find_by_username(&pool, &payload.username).await {
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
    }

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

    (
        StatusCode::OK,
        Json(AuthResponse {
            token,
            user: UserPublic {
                id: user.id,
                username: user.username,
                email: user.email,
            },
        }),
    )
        .into_response()
}

pub async fn me(auth: AuthenticatedUser, Extension(pool): Extension<SqlitePool>) -> Response {
    match user_repo::find_by_id(&pool, &auth.user_id).await {
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
