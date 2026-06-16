use cookie::Cookie;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::Span;

use crate::config::Config;
use crate::middleware::error::AuthError;
use crate::models::access_token::AccessTokenPermission;
use crate::models::authorization::{Action, Authorizer, InstanceRole, SiteRole};
use crate::repository::Repository;

type HmacSha256 = Hmac<Sha256>;

const TOKEN_PREFIX_LEN: usize = 24;

// ── Actor model ──

#[derive(Debug, Clone)]
pub struct UserActor {
    pub user_id: String,
    pub session_id: String,
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

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub actor: Actor,
    pub auth_method: AuthMethod,
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
            return Ok(AuthContext {
                actor,
                auth_method: AuthMethod::ApiKey,
            });
        }

        if let Some(token) = extract_cookie_value(parts, "token") {
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
            let user = verify_session(&token, &repository, &config.hmac_secret).await?;
            return Ok(AuthContext {
                actor: Actor::User(user),
                auth_method: AuthMethod::JwtSession,
            });
        }

        Err(AuthError::unauthorized("Authentication required"))
    }
}

// ── Cookie / header helpers ──

fn extract_cookie_value(parts: &Parts, name: &str) -> Option<String> {
    let cookie_header = parts.headers.get("cookie")?.to_str().ok()?;

    for cookie in cookie_header.split(';') {
        if let Ok(parsed) = Cookie::parse(cookie.trim())
            && parsed.name() == name
        {
            return Some(parsed.value().to_string());
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

// ── HMAC / sessions ──

pub fn compute_key_hmac(key: &str, hmac_secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(key.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub async fn verify_session(
    token: &str,
    repository: &Repository,
    hmac_secret: &str,
) -> Result<UserActor, (StatusCode, Json<AuthError>)> {
    let session = repository
        .session
        .find_active_by_hash(&compute_key_hmac(token, hmac_secret))
        .await
        .map_err(|_| AuthError::unauthorized("Authentication service unavailable"))?
        .ok_or_else(|| AuthError::unauthorized("Invalid or expired session"))?;
    let _ = repository.session.touch(&session.id).await;
    Ok(UserActor {
        user_id: session.user_id,
        session_id: session.id,
    })
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

pub fn verify_csrf(parts: &Parts, hmac_secret: &str, expected_hash: &str) -> Result<(), (StatusCode, String)> {
    let csrf_cookie =
        extract_cookie_value(parts, "csrf").ok_or((StatusCode::FORBIDDEN, "Missing CSRF cookie".to_string()))?;

    let csrf_header = extract_csrf_token(parts).ok_or((StatusCode::FORBIDDEN, "Missing CSRF header".to_string()))?;

    if compute_key_hmac(&csrf_cookie, hmac_secret) != expected_hash
        || compute_key_hmac(&csrf_header, hmac_secret) != expected_hash
    {
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

pub async fn require_site_action(
    ctx: &RequestContext,
    repository: &Repository,
    action: Action,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    match &ctx.auth.actor {
        Actor::ApiKey(key) => {
            if Authorizer::allows_api_key(key.permission.can_write(), action) {
                Ok(())
            } else {
                Err(AuthError::insufficient_permission("write"))
            }
        }
        Actor::User(user) => check_site_action_repo(repository, &user.user_id, &ctx.site_id, action).await,
    }
}

pub async fn require_user_action(
    ctx: &RequestContext,
    repository: &Repository,
    action: Action,
) -> Result<String, (StatusCode, Json<AuthError>)> {
    match &ctx.auth.actor {
        Actor::User(user) => {
            check_site_action_repo(repository, &user.user_id, &ctx.site_id, action).await?;
            Ok(user.user_id.clone())
        }
        _ => Err(AuthError::site_token_denied()),
    }
}

pub async fn require_instance_action(
    auth: &AuthContext,
    repository: &Repository,
    action: Action,
) -> Result<String, (StatusCode, Json<AuthError>)> {
    let Actor::User(user) = &auth.actor else {
        return Err(AuthError::site_token_denied());
    };
    let account = repository
        .user
        .find_by_id(&user.user_id)
        .await
        .map_err(|_| AuthError::unauthorized("Unable to load user"))?
        .ok_or_else(|| AuthError::unauthorized("User not found"))?;
    let role = account
        .instance_role
        .as_deref()
        .and_then(|value| value.parse::<InstanceRole>().ok());
    if Authorizer::allows_instance(role, action) {
        Ok(user.user_id.clone())
    } else {
        Err(AuthError::insufficient_role("instance_owner"))
    }
}

pub async fn check_site_action_repo(
    repository: &Repository,
    user_id: &str,
    site_id: &str,
    action: Action,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    // Instance operators (Owner/Admin) have full authority over every site, without
    // needing a site_members row. This is the single override point for "manage all sites".
    if let Some(account) = repository
        .user
        .find_by_id(user_id)
        .await
        .map_err(|_| AuthError::unauthorized("Unable to load user"))?
    {
        let instance_role = account
            .instance_role
            .as_deref()
            .and_then(|value| value.parse::<InstanceRole>().ok());
        if Authorizer::allows_site_as_instance(instance_role, action) {
            return Ok(());
        }
    }

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
        Some(role)
            if role
                .parse::<SiteRole>()
                .ok()
                .is_some_and(|role| Authorizer::allows_site(role, action)) =>
        {
            Ok(())
        }
        Some(_) => Err(AuthError::insufficient_role("required site role")),
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
            session_id: "session-1".into(),
        });
        assert_eq!(actor.user_id(), Some("usr-1"));
        assert!(actor.bound_site_id().is_none());
    }
}
