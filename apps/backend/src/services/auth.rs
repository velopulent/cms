use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::Cookie;
use bcrypt::{DEFAULT_COST, hash, verify};
use serde_json::json;
use thiserror::Error;
use time::Duration;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::middleware::auth::create_token;
use crate::models::user::{AuthResponse, UserPublic};
use crate::repository::error::RepositoryError;
use crate::repository::traits::UserRepository;

static EMAIL_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap());

#[derive(Clone)]
pub struct AuthService {
    user_repo: Arc<dyn UserRepository>,
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
            AuthError::UserExists => (
                StatusCode::CONFLICT,
                Json(json!({"error": "Username or email already exists"})),
            ),
            AuthError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid username or password"})),
            ),
            AuthError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))),
            AuthError::TokenError(msg) | AuthError::HashError(msg) | AuthError::DatabaseError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
            }
        };
        (status, body).into_response()
    }
}

impl AuthService {
    pub fn new(user_repo: Arc<dyn UserRepository>, jwt_secret: String, cookie_secure: bool) -> Self {
        Self {
            user_repo,
            jwt_secret,
            cookie_secure,
        }
    }

    pub async fn register(&self, username: &str, email: &str, password: &str) -> Result<UserPublic, AuthError> {
        let username = username.trim();
        let email = email.trim();
        let password = password.trim();

        let email_display = if email.is_empty() {
            "<empty>".to_string()
        } else {
            let mut chars = email.chars();
            let first = chars.next().unwrap_or('_');
            let last = chars.last().unwrap_or(first);
            format!("{}***{}", first, last)
        };
        debug!("Attempting to register user: username={}, email={}", username, email_display);

        if username.is_empty() {
            warn!("Registration failed: username is empty");
            return Err(AuthError::ValidationError("Username is required".into()));
        }

        if username.len() < 3 {
            warn!("Registration failed: username too short (length={})", username.len());
            return Err(AuthError::ValidationError(
                "Username must be at least 3 characters".into(),
            ));
        }

        if password.is_empty() {
            warn!("Registration failed: password is empty");
            return Err(AuthError::ValidationError("Password is required".into()));
        }

        if password.len() < 8 {
            warn!("Registration failed: password too short (length={})", password.len());
            return Err(AuthError::ValidationError(
                "Password must be at least 8 characters".into(),
            ));
        }

        if !EMAIL_RE.is_match(email) {
            warn!("Registration failed: invalid email format");
            return Err(AuthError::ValidationError("Invalid email address".into()));
        }

        let password_hash = hash(password, DEFAULT_COST).map_err(|e| AuthError::HashError(e.to_string()))?;

        let id = Uuid::now_v7().to_string();
        info!("Creating new user: id={}, username={}", id, username);

        match self.user_repo
            .create(&id, username, email, &password_hash)
            .await
        {
            Ok(_) => {
                info!("User registered successfully: id={}", id);
                Ok(UserPublic {
                    id,
                    username: username.to_string(),
                    email: email.to_string(),
                })
            }
            Err(e) => {
                error!("Failed to register user: id={}, error={}", id, e);
                Err(match e {
                    RepositoryError::UniqueViolation(_) => AuthError::UserExists,
                    _ => AuthError::DatabaseError(e.to_string()),
                })
            }
        }
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<(UserPublic, String), AuthError> {
        debug!("Attempting login for username={}", username);
        
        let user = self
            .user_repo
            .find_by_username(username)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::InvalidCredentials)?;

        debug!("User found for username={}, verifying password", username);
        
        match verify(password, &user.password_hash) {
            Ok(true) => {
                info!("Login successful for user: id={}, username={}", user.id, user.username);
                let token =
                    create_token(user.id.clone(), &self.jwt_secret).map_err(|e| AuthError::TokenError(e.to_string()))?;
                Ok((
                    UserPublic {
                        id: user.id,
                        username: user.username,
                        email: user.email,
                    },
                    token,
                ))
            }
            _ => {
                warn!("Login failed: invalid credentials for username={}", username);
                Err(AuthError::InvalidCredentials)
            }
        }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<Option<UserPublic>, AuthError> {
        debug!("Fetching user by id: {}", user_id);
        match self.user_repo.find_by_id(user_id).await {
            Ok(Some(user)) => {
                debug!("User found: id={}, username={}", user.id, user.username);
                Ok(Some(UserPublic {
                    id: user.id,
                    username: user.username,
                    email: user.email,
                }))
            }
            Ok(None) => {
                debug!("User not found: id={}", user_id);
                Ok(None)
            }
            Err(e) => {
                error!("Error fetching user id={}: {}", user_id, e);
                Err(AuthError::DatabaseError(e.to_string()))
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::User;
    use crate::test_helpers::InMemoryUserRepository;

    fn create_test_user() -> User {
        User {
            id: "user-123".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: bcrypt::hash("password123", bcrypt::DEFAULT_COST).unwrap(),
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    fn test_user_repo() -> Arc<InMemoryUserRepository> {
        Arc::new(InMemoryUserRepository::new())
    }

    #[tokio::test]
    async fn test_register_success() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("newuser", "new@example.com", "password123").await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "newuser");
        assert_eq!(user.email, "new@example.com");
        assert!(!user.id.is_empty());
    }

    #[tokio::test]
    async fn test_register_trims_whitespace() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth
            .register("  newuser  ", "  new@example.com  ", "  password123  ")
            .await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "newuser");
        assert_eq!(user.email, "new@example.com");
    }

    #[tokio::test]
    async fn test_register_empty_username() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("", "test@example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Username is required")));
    }

    #[tokio::test]
    async fn test_register_username_too_short() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("ab", "test@example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("at least 3 characters")));
    }

    #[tokio::test]
    async fn test_register_empty_password() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("validuser", "test@example.com", "").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Password is required")));
    }

    #[tokio::test]
    async fn test_register_password_too_short() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("validuser", "test@example.com", "short").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("at least 8 characters")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_no_at() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("validuser", "invalid-email", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_no_domain() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("validuser", "user@", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_no_tld() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("validuser", "user@example", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_with_spaces() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("validuser", "user@ example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_duplicate_username() {
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.register("testuser", "other@example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::UserExists)));
    }

    #[tokio::test]
    async fn test_login_success() {
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.login("testuser", "password123").await;
        assert!(result.is_ok());
        let (user, token) = result.unwrap();
        assert_eq!(user.username, "testuser");
        assert!(!token.is_empty());
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.login("testuser", "wrongpassword").await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_login_user_not_found() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.login("nonexistent", "password123").await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_get_user_found() {
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.get_user("user-123").await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert!(user.is_some());
        assert_eq!(user.unwrap().username, "testuser");
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let result = auth.get_user("nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_cookie_secure_getter() {
        let user_repo = test_user_repo();
        let auth_secure = AuthService::new(user_repo.clone(), "secret".to_string(), true);
        let auth_insecure = AuthService::new(user_repo, "secret".to_string(), false);

        assert!(auth_secure.cookie_secure());
        assert!(!auth_insecure.cookie_secure());
    }

    #[test]
    fn test_auth_error_into_response() {
        assert_eq!(
            AuthError::ValidationError("bad input".into()).into_response().status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AuthError::UserExists.into_response().status(),
            axum::http::StatusCode::CONFLICT
        );
        assert_eq!(
            AuthError::InvalidCredentials.into_response().status(),
            axum::http::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AuthError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            AuthError::TokenError("tok".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            AuthError::HashError("hash".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            AuthError::DatabaseError("db".into()).into_response().status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn test_build_auth_cookies_response() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), true);

        let user = UserPublic {
            id: "user-123".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
        };

        let response = auth.build_auth_cookies_response(user, "jwt-token");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(response.headers().contains_key(axum::http::header::SET_COOKIE));
    }

    #[tokio::test]
    async fn test_build_logout_response() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let response = auth.build_logout_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(response.headers().contains_key(axum::http::header::SET_COOKIE));
    }

    #[tokio::test]
    async fn test_build_register_response() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo, "secret".to_string(), false);

        let user = UserPublic {
            id: "user-123".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
        };

        let response = auth.build_register_response(user, "jwt-token");
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_register_multiple_users() {
        let user_repo = test_user_repo();
        let auth = AuthService::new(user_repo.clone(), "secret".to_string(), false);

        let result1 = auth.register("user1", "user1@example.com", "password123").await;
        assert!(result1.is_ok());

        let result2 = auth.register("user2", "user2@example.com", "password123").await;
        assert!(result2.is_ok());

        let result3 = auth.register("user3", "user3@example.com", "password123").await;
        assert!(result3.is_ok());

        let user1 = auth.login("user1", "password123").await;
        assert!(user1.is_ok());
    }
}
