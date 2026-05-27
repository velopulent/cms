use std::collections::HashSet;

use cookie::Cookie;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use hmac::{Hmac, Mac};
use hmac::digest::KeyInit;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation, decode, encode};
use sha2::Sha256;
use tracing::Span;

use crate::config::Config;
use crate::middleware::error::AuthError;
use crate::models::access_token::AccessTokenPermission;
use crate::repository::Repository;

type HmacSha256 = Hmac<Sha256>;

const TOKEN_PREFIX_LEN: usize = 24;

// ── Actor model ──

#[derive(Debug, Clone)]
pub struct UserActor {
    pub user_id: String,
}

#[derive(Debug, Clone)]
pub struct ApiKeyActor {
    pub token_id: String,
    pub site_id: String,
    pub permission: AccessTokenPermission,
}

#[derive(Debug, Clone)]
pub enum Actor {
    User(UserActor),
    ApiKey(ApiKeyActor),
}

impl Actor {
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::User(u) => Some(&u.user_id),
            _ => None,
        }
    }

    pub fn bound_site_id(&self) -> Option<&str> {
        match self {
            Self::ApiKey(k) => Some(&k.site_id),
            _ => None,
        }
    }
}

// ── Auth context ──

#[derive(Debug, Clone)]
pub enum AuthMethod {
    JwtSession,
    ApiKey,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum Scope {
    SiteRead,
    FilesRead,
    FilesWrite,
    CollectionsRead,
    CollectionsWrite,
    EntriesRead,
    EntriesWrite,
    WebhooksRead,
    WebhooksWrite,
}

#[derive(Debug, Clone)]
pub struct ScopeSet(pub HashSet<Scope>);

impl ScopeSet {
    pub fn from_permission(perm: &AccessTokenPermission) -> Self {
        let mut set = HashSet::new();
        set.insert(Scope::SiteRead);
        set.insert(Scope::FilesRead);
        set.insert(Scope::CollectionsRead);
        set.insert(Scope::EntriesRead);
        set.insert(Scope::WebhooksRead);
        if perm.can_write() {
            set.insert(Scope::FilesWrite);
            set.insert(Scope::CollectionsWrite);
            set.insert(Scope::EntriesWrite);
            set.insert(Scope::WebhooksWrite);
        }
        Self(set)
    }

    pub fn all() -> Self {
        Self(HashSet::from([
            Scope::SiteRead,
            Scope::FilesRead,
            Scope::FilesWrite,
            Scope::CollectionsRead,
            Scope::CollectionsWrite,
            Scope::EntriesRead,
            Scope::EntriesWrite,
            Scope::WebhooksRead,
            Scope::WebhooksWrite,
        ]))
    }

    pub fn allows(&self, scope: &Scope) -> bool {
        self.0.contains(scope)
    }
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub actor: Actor,
    pub auth_method: AuthMethod,
    pub scopes: ScopeSet,
}

// ── Request context ──

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub site_id: String,
    pub auth: AuthContext,
}

impl<S> FromRequestParts<S> for RequestContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<AuthError>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestContext>()
            .cloned()
            .ok_or_else(|| AuthError::unauthorized("Missing request context"))
    }
}

/// Extract AuthContext directly from request (for file-serving routes without middleware).
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<AuthError>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(ctx) = parts.extensions.get::<AuthContext>() {
            return Ok(ctx.clone());
        }

        if let Some(token) = extract_bearer_token(parts) {
            let repository = parts
                .extensions
                .get::<Repository>()
                .cloned()
                .ok_or_else(|| AuthError::unauthorized("Server configuration error"))?;
            let config = parts
                .extensions
                .get::<Config>()
                .cloned()
                .ok_or_else(|| AuthError::unauthorized("Server configuration error"))?;
            let actor = verify_access_token(&token, &repository, &config.hmac_secret).await?;
            let scopes = match &actor {
                Actor::ApiKey(k) => ScopeSet::from_permission(&k.permission),
                _ => ScopeSet::all(),
            };
            return Ok(AuthContext {
                actor,
                auth_method: AuthMethod::ApiKey,
                scopes,
            });
        }

        if let Some(token) = extract_cookie_value(parts, "token") {
            let config = parts
                .extensions
                .get::<Config>()
                .cloned()
                .ok_or_else(|| AuthError::unauthorized("Server configuration error"))?;
            let claims = verify_token(&token, &config.jwt_secret)
                .map_err(|_| AuthError::unauthorized("Invalid session"))?;
            return Ok(AuthContext {
                actor: Actor::User(UserActor {
                    user_id: claims.sub,
                }),
                auth_method: AuthMethod::JwtSession,
                scopes: ScopeSet::all(),
            });
        }

        Err(AuthError::unauthorized("Authentication required"))
    }
}

// ── Cookie / header helpers ──

fn extract_cookie_value(parts: &Parts, name: &str) -> Option<String> {
    let cookie_header = parts.headers.get("cookie")?.to_str().ok()?;

    for cookie in cookie_header.split(';') {
        if let Ok(parsed) = Cookie::parse(cookie.trim()) {
            if parsed.name() == name {
                return Some(parsed.value().to_string());
            }
        }
    }

    None
}

fn extract_bearer_token(parts: &Parts) -> Option<String> {
    let auth_header = parts.headers.get("Authorization")?.to_str().ok()?;
    auth_header
        .strip_prefix("Bearer ")
        .map(|token| token.trim().to_string())
}

fn extract_csrf_token(parts: &Parts) -> Option<String> {
    let header = parts.headers.get("x-csrf-token")?.to_str().ok()?;
    Some(header.to_string())
}

// ── HMAC / JWT ──

pub fn compute_key_hmac(key: &str, hmac_secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(key.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn create_token(user_id: String, jwt_secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = crate::models::user::Claims {
        sub: user_id,
        exp: expiration,
    };

    encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
}

pub fn verify_token(token: &str, jwt_secret: &str) -> Result<crate::models::user::Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let data = decode::<crate::models::user::Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )?;

    Ok(data.claims)
}

// ── Access token verification ──

pub(crate) async fn verify_access_token(
    token: &str,
    repository: &Repository,
    hmac_secret: &str,
) -> Result<Actor, (StatusCode, Json<AuthError>)> {
    if !token.starts_with("cms_site_") {
        return Err(AuthError::unauthorized("Invalid access token"));
    }

    let prefix: String = token.chars().take(TOKEN_PREFIX_LEN).collect();

    let keys = repository
        .access_token
        .find_by_prefix(&prefix)
        .await
        .map_err(|_| AuthError::unauthorized("Internal server error"))?;

    let token_hmac = compute_key_hmac(token, hmac_secret);

    for (token_id, site_id, stored_hash, stored_hmac, expires_at, revoked_at, permission) in keys {
        if let Some(ref stored) = stored_hmac {
            if stored != &token_hmac {
                continue;
            }
        } else if !bcrypt::verify(token, &stored_hash).unwrap_or(false) {
            continue;
        }

        if revoked_at.is_some() {
            return Err(AuthError::unauthorized("Access token has been revoked"));
        }

        if !is_token_not_expired(expires_at.as_deref()) {
            return Err(AuthError::unauthorized("Access token has expired"));
        }

        let permission = permission
            .parse::<AccessTokenPermission>()
            .map_err(|_| AuthError::unauthorized("Invalid access token"))?;

        let _ = repository.access_token.update_last_used(&token_id).await;

        Span::current().record("site_id", tracing::field::display(&site_id));
        return Ok(Actor::ApiKey(ApiKeyActor {
            token_id,
            site_id,
            permission,
        }));
    }

    tracing::warn!(prefix = %prefix, "Invalid access token attempt");
    Err(AuthError::unauthorized("Invalid access token"))
}

// ── CSRF ──

pub fn verify_csrf(parts: &Parts, config: &Config) -> Result<(), (StatusCode, String)> {
    if !config.cookie_secure {
        return Ok(());
    }

    let csrf_cookie =
        extract_cookie_value(parts, "csrf").ok_or((StatusCode::FORBIDDEN, "Missing CSRF cookie".to_string()))?;

    let csrf_header = extract_csrf_token(parts).ok_or((StatusCode::FORBIDDEN, "Missing CSRF header".to_string()))?;

    if csrf_cookie != csrf_header {
        tracing::warn!("CSRF mismatch detected");
        return Err((StatusCode::FORBIDDEN, "CSRF token mismatch".to_string()));
    }

    Ok(())
}

pub fn is_token_not_expired(expires_at: Option<&str>) -> bool {
    let exp = match expires_at {
        Some(v) => v,
        None => return true,
    };

    let now = chrono::Utc::now();

    match chrono::DateTime::parse_from_rfc3339(exp) {
        Ok(dt) => dt >= now,
        Err(e) => {
            tracing::warn!(error = %e, expires_at = %exp, "Invalid expiry format");
            false
        }
    }
}

// ── Authorization helpers ──

pub async fn require_site_scope(
    ctx: &RequestContext,
    repository: &Repository,
    scope: &Scope,
    jwt_min_role: &str,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    match &ctx.auth.actor {
        Actor::ApiKey(_) => {
            if ctx.auth.scopes.allows(scope) {
                Ok(())
            } else {
                Err(AuthError::insufficient_permission("write"))
            }
        }
        Actor::User(user) => {
            check_site_access_repo(repository, &user.user_id, &ctx.site_id, jwt_min_role).await
        }
    }
}

pub async fn require_user_role(
    ctx: &RequestContext,
    repository: &Repository,
    min_role: &str,
) -> Result<String, (StatusCode, Json<AuthError>)> {
    match &ctx.auth.actor {
        Actor::User(user) => {
            check_site_access_repo(repository, &user.user_id, &ctx.site_id, min_role).await?;
            Ok(user.user_id.clone())
        }
        _ => Err(AuthError::site_token_denied()),
    }
}

pub async fn check_site_access_repo(
    repository: &Repository,
    user_id: &str,
    site_id: &str,
    min_role: &str,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    let role_order = |r: &str| match r {
        "owner" => 4,
        "admin" => 3,
        "editor" => 2,
        "viewer" => 1,
        _ => 0,
    };

    let min_level = role_order(min_role);

    let role = repository.user.get_role(user_id, site_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "internal_error".into(),
                message: e.to_string(),
            }),
        )
    })?;

    match role {
        Some(r) if role_order(&r) >= min_level => Ok(()),
        Some(_) => Err(AuthError::insufficient_role(min_role)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "not_found".into(),
                message: "Site not found".into(),
            }),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::access_token::AccessTokenPermission;

    #[test]
    fn test_create_and_verify_token() {
        let user_id = "test-user-123";
        let secret = "test-jwt-secret";

        let token = create_token(user_id.to_string(), secret).expect("Token creation should succeed");
        assert!(!token.is_empty());

        let claims = verify_token(&token, secret).expect("Token verification should succeed");
        assert_eq!(claims.sub, user_id);
    }

    #[test]
    fn test_scope_set_from_read_permission() {
        let scopes = ScopeSet::from_permission(&AccessTokenPermission::Read);
        assert!(scopes.allows(&Scope::FilesRead));
        assert!(!scopes.allows(&Scope::FilesWrite));
    }

    #[test]
    fn test_scope_set_from_write_permission() {
        let scopes = ScopeSet::from_permission(&AccessTokenPermission::Write);
        assert!(scopes.allows(&Scope::FilesRead));
        assert!(scopes.allows(&Scope::FilesWrite));
    }

    #[test]
    fn test_scope_set_all() {
        let scopes = ScopeSet::all();
        assert!(scopes.allows(&Scope::FilesRead));
        assert!(scopes.allows(&Scope::FilesWrite));
        assert!(scopes.allows(&Scope::EntriesWrite));
        assert!(scopes.allows(&Scope::WebhooksWrite));
    }

    #[test]
    fn test_is_token_not_expired() {
        let now = chrono::Utc::now();
        let future = now + chrono::Duration::hours(1);
        let past = now - chrono::Duration::hours(1);

        assert!(is_token_not_expired(None));
        assert!(is_token_not_expired(Some(&future.to_rfc3339())));
        assert!(!is_token_not_expired(Some(&past.to_rfc3339())));
        assert!(!is_token_not_expired(Some("invalid-date")));
    }

    #[test]
    fn test_compute_key_hmac_deterministic() {
        let key = "cms_site_abcdefgh_1234567890123456789012";
        let secret = "test-hmac-secret";
        let h1 = compute_key_hmac(key, secret);
        let h2 = compute_key_hmac(key, secret);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_api_key_actor_bound_site() {
        let actor = Actor::ApiKey(ApiKeyActor {
            token_id: "tok-1".into(),
            site_id: "site-42".into(),
            permission: AccessTokenPermission::Read,
        });
        assert_eq!(actor.bound_site_id(), Some("site-42"));
        assert!(actor.user_id().is_none());
    }

    #[test]
    fn test_user_actor_user_id() {
        let actor = Actor::User(UserActor {
            user_id: "usr-1".into(),
        });
        assert_eq!(actor.user_id(), Some("usr-1"));
        assert!(actor.bound_site_id().is_none());
    }
}
