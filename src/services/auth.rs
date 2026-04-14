use std::sync::Arc;

use axum_extra::extract::cookie::Cookie;
use axum::http::{HeaderValue, StatusCode};
use axum::Json;
use axum::response::{IntoResponse, Response};
use bcrypt::{DEFAULT_COST, hash, verify};
use regex::Regex;
use serde_json::json;
use thiserror::Error;
use time::Duration;
use uuid::Uuid;

use crate::middleware::auth::create_token;
use crate::models::user::{AuthResponse, UserPublic};
use crate::repository::Repository;
use crate::repository::error::RepositoryError;

static EMAIL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap());

#[derive(Clone)]
pub struct AuthService {
    repository: Arc<Repository>,
    jwt_secret: String,
    cookie_secure: bool,
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("User already exists")]
    UserExists,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("User not found")]
    NotFound,

    #[error("Token error: {0}")]
    TokenError(String),

    #[error("Password hash error: {0}")]
    HashError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl AuthError {
    pub fn into_response(self) -> Response {
        let (status, body) = match self {
            AuthError::ValidationError(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            AuthError::UserExists => (StatusCode::CONFLICT, Json(json!({"error": "Username or email already exists"}))),
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid username or password"}))),
            AuthError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))),
            AuthError::TokenError(msg) | AuthError::HashError(msg) | AuthError::DatabaseError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
            }
        };
        (status, body).into_response()
    }
}

impl AuthService {
    pub fn new(repository: Arc<Repository>, jwt_secret: String, cookie_secure: bool) -> Self {
        Self {
            repository,
            jwt_secret,
            cookie_secure,
        }
    }

    pub async fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<UserPublic, AuthError> {
        let username = username.trim();
        let email = email.trim();
        let password = password.trim();

        if username.is_empty() {
            return Err(AuthError::ValidationError("Username is required".into()));
        }

        if username.len() < 3 {
            return Err(AuthError::ValidationError(
                "Username must be at least 3 characters".into(),
            ));
        }

        if password.is_empty() {
            return Err(AuthError::ValidationError("Password is required".into()));
        }

        if password.len() < 8 {
            return Err(AuthError::ValidationError(
                "Password must be at least 8 characters".into(),
            ));
        }

        if !EMAIL_RE.is_match(email) {
            return Err(AuthError::ValidationError("Invalid email address".into()));
        }

        let password_hash = hash(password, DEFAULT_COST).map_err(|e| AuthError::HashError(e.to_string()))?;

        let id = Uuid::now_v7().to_string();

        self.repository
            .user
            .create(&id, username, email, &password_hash)
            .await
            .map_err(|e| match e {
                RepositoryError::UniqueViolation(_) => AuthError::UserExists,
                _ => AuthError::DatabaseError(e.to_string()),
            })?;

        Ok(UserPublic {
            id,
            username: username.to_string(),
            email: email.to_string(),
        })
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<(UserPublic, String), AuthError> {
        let user = self
            .repository
            .user
            .find_by_username(username)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::InvalidCredentials)?;

        match verify(password, &user.password_hash) {
            Ok(true) => {}
            _ => return Err(AuthError::InvalidCredentials),
        }

        let token = create_token(user.id.clone(), &self.jwt_secret)
            .map_err(|e| AuthError::TokenError(e.to_string()))?;

        Ok((
            UserPublic {
                id: user.id,
                username: user.username,
                email: user.email,
            },
            token,
        ))
    }

    pub async fn get_user(&self, user_id: &str) -> Result<Option<UserPublic>, AuthError> {
        self.repository
            .user
            .find_by_id(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))
            .map(|opt| opt.map(|u| UserPublic {
                id: u.id,
                username: u.username,
                email: u.email,
            }))
    }

    pub fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }

    pub fn build_auth_cookies_response(&self, user: UserPublic, jwt: &str) -> Response {
        let token_cookie = Cookie::build(("token", jwt.to_string()))
            .http_only(true)
            .secure(self.cookie_secure)
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
            .path("/")
            .max_age(Duration::hours(24))
            .build();

        let csrf_token = Uuid::now_v7().to_string();
        let csrf_cookie = Cookie::build(("csrf", csrf_token.clone()))
            .http_only(false)
            .secure(self.cookie_secure)
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
            .path("/")
            .max_age(Duration::hours(24))
            .build();

        let mut response = (StatusCode::OK, Json(AuthResponse { user })).into_response();

        if let Ok(val) = HeaderValue::from_str(&token_cookie.to_string()) {
            response.headers_mut().insert(axum::http::header::SET_COOKIE, val);
        }
        if let Ok(val) = HeaderValue::from_str(&csrf_cookie.to_string()) {
            response.headers_mut().append(axum::http::header::SET_COOKIE, val);
        }

        response
    }

    pub fn build_logout_response(&self) -> Response {
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

        let mut response = (StatusCode::OK, Json(json!({"message": "Logged out"}))).into_response();

        if let Ok(val) = HeaderValue::from_str(&clear_token.to_string()) {
            response.headers_mut().insert(axum::http::header::SET_COOKIE, val);
        }
        if let Ok(val) = HeaderValue::from_str(&clear_csrf.to_string()) {
            response.headers_mut().append(axum::http::header::SET_COOKIE, val);
        }

        response
    }

    pub fn build_register_response(&self, user: UserPublic, jwt: &str) -> Response {
        let mut response = self.build_auth_cookies_response(user, jwt);
        *response.status_mut() = StatusCode::CREATED;
        response
    }
}
