use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::Cookie;
use bcrypt::{hash, verify};
use serde_json::json;
use thiserror::Error;
use time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::middleware::auth::compute_key_hmac;
use crate::models::session::SessionSummary;
use crate::models::user::{AuthResponse, UserPublic};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{SessionRepository, UserRepository};

static EMAIL_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap());

/// Validate an optional instance role string from an API payload.
/// Accepts `"instance_owner"`, `"instance_admin"`, or `None`; rejects anything else.
fn normalize_instance_role(role: Option<&str>) -> Result<Option<&str>, AuthError> {
    match role {
        None => Ok(None),
        Some("instance_owner") => Ok(Some("instance_owner")),
        Some("instance_admin") => Ok(Some("instance_admin")),
        Some(other) => Err(AuthError::ValidationError(format!("Invalid instance role '{other}'"))),
    }
}

#[derive(Clone)]
pub struct AuthService {
    user_repo: Arc<dyn UserRepository>,
    session_repo: Arc<dyn SessionRepository>,
    hmac_secret: String,
    cookie_secure: bool,
    session_lifetime_hours: i64,
    public_registration_enabled: bool,
    bcrypt_cost: u32,
    settings: Option<crate::services::settings::SettingsService>,
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("User already exists")]
    UserExists,

    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Public registration is disabled")]
    RegistrationDisabled,

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
            AuthError::UserExists => (StatusCode::CONFLICT, Json(json!({"error": "Email already exists"}))),
            AuthError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid email or password"})),
            ),
            AuthError::RegistrationDisabled => (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "Public registration is disabled"})),
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
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        session_repo: Arc<dyn SessionRepository>,
        hmac_secret: String,
        cookie_secure: bool,
        session_lifetime_hours: i64,
        public_registration_enabled: bool,
        bcrypt_cost: u32,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            hmac_secret,
            cookie_secure,
            session_lifetime_hours,
            public_registration_enabled,
            bcrypt_cost,
            settings: None,
        }
    }

    pub fn with_settings(mut self, settings: crate::services::settings::SettingsService) -> Self {
        self.settings = Some(settings);
        self
    }

    fn runtime_values(&self) -> (bool, i64, bool) {
        self.settings.as_ref().map_or(
            (
                self.cookie_secure,
                self.session_lifetime_hours,
                self.public_registration_enabled,
            ),
            |service| {
                let settings = service.current();
                (
                    settings.security.secure_cookies,
                    settings.general.session_lifetime_hours as i64,
                    settings.general.public_registration,
                )
            },
        )
    }

    pub async fn register(&self, name: &str, email: &str, password: &str) -> Result<UserPublic, AuthError> {
        let user_count = self
            .user_repo
            .count()
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        if user_count > 0 && !self.runtime_values().2 {
            return Err(AuthError::RegistrationDisabled);
        }
        let name = name.trim();
        let email = email.trim();
        let password = password.trim();

        debug!("Attempting to register user");

        // `name` is now a human display name (e.g. "John Doe"): non-unique, spaces
        // allowed. The login identity is the email, validated below.
        if name.is_empty() {
            warn!("Registration failed: name is empty");
            return Err(AuthError::ValidationError("Name is required".into()));
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

        let password_hash = hash(password, self.bcrypt_cost).map_err(|e| AuthError::HashError(e.to_string()))?;

        let id = Uuid::now_v7().to_string();
        info!("Creating new user: id={}", id);

        match self.user_repo.create(&id, name, email, &password_hash).await {
            Ok(_) => {
                let instance_role = if user_count == 0 {
                    self.user_repo
                        .set_instance_role(&id, Some("instance_owner"))
                        .await
                        .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
                    Some("instance_owner".to_string())
                } else {
                    None
                };
                info!("User registered successfully: id={}", id);
                Ok(UserPublic {
                    id,
                    name: name.to_string(),
                    email: email.to_string(),
                    instance_role,
                    must_change_password: false,
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

    pub async fn login(&self, email: &str, password: &str) -> Result<(UserPublic, String, String), AuthError> {
        let email = email.trim();
        let password = password.trim();
        debug!("Attempting login for email={}", email);

        let user = self
            .user_repo
            .find_by_email(email)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::InvalidCredentials)?;

        debug!("User found for email={}, verifying password", email);

        match verify(password, &user.password_hash) {
            Ok(true) => {
                info!("Login successful for user: id={}, name={}", user.id, user.name);
                let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
                let csrf_token = Uuid::new_v4().to_string();
                let expires_at = (chrono::Utc::now() + chrono::Duration::hours(self.runtime_values().1)).to_rfc3339();
                self.session_repo
                    .create(
                        &Uuid::now_v7().to_string(),
                        &user.id,
                        &compute_key_hmac(&token, &self.hmac_secret),
                        &compute_key_hmac(&csrf_token, &self.hmac_secret),
                        &expires_at,
                    )
                    .await
                    .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
                Ok((
                    UserPublic {
                        id: user.id,
                        name: user.name,
                        email: user.email,
                        instance_role: user.instance_role,
                        must_change_password: user.must_change_password,
                    },
                    token,
                    csrf_token,
                ))
            }
            _ => {
                warn!("Login failed: invalid credentials for email={}", email);
                Err(AuthError::InvalidCredentials)
            }
        }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<Option<UserPublic>, AuthError> {
        debug!("Fetching user by id: {}", user_id);
        match self.user_repo.find_by_id(user_id).await {
            Ok(Some(user)) => {
                debug!("User found: id={}, name={}", user.id, user.name);
                Ok(Some(UserPublic {
                    id: user.id,
                    name: user.name,
                    email: user.email,
                    instance_role: user.instance_role,
                    must_change_password: user.must_change_password,
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

    pub async fn list_users(&self) -> Result<Vec<UserPublic>, AuthError> {
        let users = self
            .user_repo
            .list()
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(users
            .into_iter()
            .map(|user| UserPublic {
                id: user.id,
                name: user.name,
                email: user.email,
                instance_role: user.instance_role,
                must_change_password: user.must_change_password,
            })
            .collect())
    }

    pub async fn create_managed_user(
        &self,
        name: &str,
        email: &str,
        temporary_password: &str,
        instance_role: Option<&str>,
    ) -> Result<UserPublic, AuthError> {
        let temporary_password = temporary_password.trim();
        if temporary_password.len() < 8 {
            return Err(AuthError::ValidationError(
                "Temporary password must be at least 8 characters".into(),
            ));
        }
        let name = name.trim();
        let email = email.trim();
        // `name` is a display name (non-unique, spaces allowed); only the email is the
        // login identity and must be valid + unique.
        if name.is_empty() || !EMAIL_RE.is_match(email) {
            return Err(AuthError::ValidationError("Invalid name or email".into()));
        }
        // Validate the role before any DB write so a bad role never leaves an orphan user.
        let instance_role = normalize_instance_role(instance_role)?;
        let id = Uuid::now_v7().to_string();
        let password_hash =
            hash(temporary_password, self.bcrypt_cost).map_err(|e| AuthError::HashError(e.to_string()))?;
        self.user_repo
            .create(&id, name, email, &password_hash)
            .await
            .map_err(|error| match error {
                RepositoryError::UniqueViolation(_) => AuthError::UserExists,
                other => AuthError::DatabaseError(other.to_string()),
            })?;
        self.user_repo
            .set_instance_role(&id, instance_role)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        self.user_repo
            .update_password(&id, &password_hash, true)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(UserPublic {
            id,
            name: name.to_string(),
            email: email.to_string(),
            instance_role: instance_role.map(ToString::to_string),
            must_change_password: true,
        })
    }

    pub async fn set_instance_role(&self, user_id: &str, role: Option<&str>) -> Result<(), AuthError> {
        let role = normalize_instance_role(role)?;
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::NotFound)?;
        // Guard the last instance owner from being demoted away.
        let removing_owner = user.instance_role.as_deref() == Some("instance_owner") && role != Some("instance_owner");
        if removing_owner
            && self
                .user_repo
                .count_instance_owners()
                .await
                .map_err(|e| AuthError::DatabaseError(e.to_string()))?
                <= 1
        {
            return Err(AuthError::ValidationError(
                "At least one instance owner is required".into(),
            ));
        }
        self.user_repo
            .set_instance_role(user_id, role)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Operator-driven update of another user's display name and email.
    pub async fn update_user_profile(&self, user_id: &str, name: &str, email: &str) -> Result<(), AuthError> {
        let name = name.trim();
        let email = email.trim();
        if name.is_empty() || !EMAIL_RE.is_match(email) {
            return Err(AuthError::ValidationError("Invalid name or email".into()));
        }
        self.user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::NotFound)?;
        self.user_repo
            .update_profile(user_id, name, email)
            .await
            .map_err(|error| match error {
                RepositoryError::UniqueViolation(_) => AuthError::UserExists,
                other => AuthError::DatabaseError(other.to_string()),
            })?;
        Ok(())
    }

    /// Self-service update of the signed-in user's own display name.
    pub async fn update_self_name(&self, user_id: &str, name: &str) -> Result<UserPublic, AuthError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AuthError::ValidationError("Name is required".into()));
        }
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::NotFound)?;
        self.user_repo
            .update_profile(user_id, name, &user.email)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(UserPublic {
            id: user.id,
            name: name.to_string(),
            email: user.email,
            instance_role: user.instance_role,
            must_change_password: user.must_change_password,
        })
    }

    /// Operator-driven password reset; forces a change on the user's next login.
    pub async fn admin_set_password(&self, user_id: &str, new_password: &str) -> Result<(), AuthError> {
        let new_password = new_password.trim();
        if new_password.len() < 8 {
            return Err(AuthError::ValidationError(
                "Password must be at least 8 characters".into(),
            ));
        }
        self.user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::NotFound)?;
        let password_hash = hash(new_password, self.bcrypt_cost).map_err(|e| AuthError::HashError(e.to_string()))?;
        self.user_repo
            .update_password(user_id, &password_hash, true)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Delete a user; the last instance owner cannot be removed.
    pub async fn delete_user(&self, user_id: &str) -> Result<(), AuthError> {
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::NotFound)?;
        if user.instance_role.as_deref() == Some("instance_owner")
            && self
                .user_repo
                .count_instance_owners()
                .await
                .map_err(|e| AuthError::DatabaseError(e.to_string()))?
                <= 1
        {
            return Err(AuthError::ValidationError(
                "At least one instance owner is required".into(),
            ));
        }
        self.user_repo
            .delete(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub fn cookie_secure(&self) -> bool {
        self.runtime_values().0
    }

    pub fn build_auth_cookies_response(&self, user: UserPublic, token: &str, csrf_token: &str) -> Response {
        let (cookie_secure, session_lifetime_hours, _) = self.runtime_values();
        let token_cookie = Cookie::build(("token", token.to_string()))
            .http_only(true)
            .secure(cookie_secure)
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
            .path("/")
            .max_age(Duration::hours(session_lifetime_hours))
            .build();

        let csrf_cookie = Cookie::build(("csrf", csrf_token.to_string()))
            .http_only(false)
            .secure(cookie_secure)
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
            .path("/")
            .max_age(Duration::hours(session_lifetime_hours))
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

    pub fn build_register_response(&self, user: UserPublic, token: &str, csrf_token: &str) -> Response {
        let mut response = self.build_auth_cookies_response(user, token, csrf_token);
        *response.status_mut() = StatusCode::CREATED;
        response
    }

    pub async fn logout(&self, session_id: &str, user_id: &str) -> Result<(), AuthError> {
        self.session_repo
            .revoke(session_id, user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub async fn revoke_all_sessions(&self, user_id: &str) -> Result<u64, AuthError> {
        self.session_repo
            .revoke_all(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))
    }

    pub async fn list_sessions(
        &self,
        user_id: &str,
        current_session_id: &str,
    ) -> Result<Vec<SessionSummary>, AuthError> {
        let sessions = self
            .session_repo
            .list(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(sessions
            .into_iter()
            .map(|session| SessionSummary {
                current: session.id == current_session_id,
                id: session.id,
                created_at: session.created_at,
                expires_at: session.expires_at,
                last_seen_at: session.last_seen_at,
            })
            .collect())
    }

    pub async fn change_password(
        &self,
        user_id: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        let current_password = current_password.trim();
        let new_password = new_password.trim();
        if new_password.len() < 8 {
            return Err(AuthError::ValidationError(
                "Password must be at least 8 characters".into(),
            ));
        }
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::NotFound)?;
        if !verify(current_password, &user.password_hash).unwrap_or(false) {
            return Err(AuthError::InvalidCredentials);
        }
        let password_hash = hash(new_password, self.bcrypt_cost).map_err(|e| AuthError::HashError(e.to_string()))?;
        self.user_repo
            .update_password(user_id, &password_hash, false)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        self.session_repo
            .revoke_all(user_id)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::User;
    use crate::test_helpers::{InMemorySessionRepository, InMemoryUserRepository};

    fn create_test_user() -> User {
        User {
            id: "user-123".to_string(),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash: bcrypt::hash("password123", bcrypt::DEFAULT_COST).unwrap(),
            instance_role: None,
            must_change_password: false,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    fn test_user_repo() -> Arc<InMemoryUserRepository> {
        Arc::new(InMemoryUserRepository::new())
    }

    fn test_auth(user_repo: Arc<InMemoryUserRepository>, cookie_secure: bool) -> AuthService {
        AuthService::new(
            user_repo,
            Arc::new(InMemorySessionRepository::new()),
            "secret".to_string(),
            cookie_secure,
            24,
            true,
            bcrypt::DEFAULT_COST,
        )
    }

    #[tokio::test]
    async fn test_register_success() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("newuser", "new@example.com", "password123").await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.name, "newuser");
        assert_eq!(user.email, "new@example.com");
        assert!(!user.id.is_empty());
    }

    #[tokio::test]
    async fn test_register_trims_whitespace() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth
            .register("  newuser  ", "  new@example.com  ", "  password123  ")
            .await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.name, "newuser");
        assert_eq!(user.email, "new@example.com");
    }

    #[tokio::test]
    async fn test_register_empty_name() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("", "test@example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Name is required")));
    }

    #[tokio::test]
    async fn test_register_short_display_name_allowed() {
        // `name` is a display name now: short, non-unique values are fine.
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("ab", "test@example.com", "password123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_register_empty_password() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("validuser", "test@example.com", "").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Password is required")));
    }

    #[tokio::test]
    async fn test_register_password_too_short() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("validuser", "test@example.com", "short").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("at least 8 characters")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_no_at() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("validuser", "invalid-email", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_no_domain() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("validuser", "user@", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_no_tld() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("validuser", "user@example", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_invalid_email_with_spaces() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.register("validuser", "user@ example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::ValidationError(msg)) if msg.contains("Invalid email")));
    }

    #[tokio::test]
    async fn test_register_duplicate_email() {
        // Email is the unique login identity; the display name may collide.
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = test_auth(user_repo, false);

        let result = auth.register("Another Name", "test@example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::UserExists)));
    }

    #[tokio::test]
    async fn test_login_success() {
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = test_auth(user_repo, false);

        let result = auth.login("test@example.com", "password123").await;
        assert!(result.is_ok());
        let (user, token, csrf_token) = result.unwrap();
        assert_eq!(user.name, "testuser");
        assert!(!token.is_empty());
        assert!(!csrf_token.is_empty());
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = test_auth(user_repo, false);

        let result = auth.login("test@example.com", "wrongpassword").await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_login_user_not_found() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.login("nonexistent@example.com", "password123").await;
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_get_user_found() {
        let user_repo = test_user_repo();
        user_repo.add_user(create_test_user());
        let auth = test_auth(user_repo, false);

        let result = auth.get_user("user-123").await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert!(user.is_some());
        assert_eq!(user.unwrap().name, "testuser");
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let result = auth.get_user("nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_cookie_secure_getter() {
        let user_repo = test_user_repo();
        let auth_secure = test_auth(user_repo.clone(), true);
        let auth_insecure = test_auth(user_repo, false);

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
        let auth = test_auth(user_repo, true);

        let user = UserPublic {
            id: "user-123".to_string(),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            instance_role: None,
            must_change_password: false,
        };

        let response = auth.build_auth_cookies_response(user, "session-token", "csrf-token");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(response.headers().contains_key(axum::http::header::SET_COOKIE));
    }

    #[tokio::test]
    async fn test_build_logout_response() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let response = auth.build_logout_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert!(response.headers().contains_key(axum::http::header::SET_COOKIE));
    }

    #[tokio::test]
    async fn test_build_register_response() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo, false);

        let user = UserPublic {
            id: "user-123".to_string(),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            instance_role: None,
            must_change_password: false,
        };

        let response = auth.build_register_response(user, "session-token", "csrf-token");
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_register_multiple_users() {
        let user_repo = test_user_repo();
        let auth = test_auth(user_repo.clone(), false);

        let result1 = auth.register("user1", "user1@example.com", "password123").await;
        assert!(result1.is_ok());

        let result2 = auth.register("user2", "user2@example.com", "password123").await;
        assert!(result2.is_ok());

        let result3 = auth.register("user3", "user3@example.com", "password123").await;
        assert!(result3.is_ok());

        let user1 = auth.login("user1@example.com", "password123").await;
        assert!(user1.is_ok());
    }
}
