use std::collections::BTreeSet;
use std::str::FromStr;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use hmac::{Hmac, Mac};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation, decode, encode};
use sha2::Sha256;
use tracing::Span;

use crate::config::Config;
use crate::middleware::error::AuthError;
use crate::models::access_token::AccessTokenKind;
use crate::models::user::Claims;
use crate::repository::Repository;

type HmacSha256 = Hmac<Sha256>;

pub const HEADER_SITE_ID: &str = "x-cms-site-id";
pub const SCOPE_SITES_READ: &str = "sites:read";
pub const SCOPE_SITES_WRITE: &str = "sites:write";
pub const SCOPE_SITES_DELETE: &str = "sites:delete";
pub const SCOPE_MEMBERS_READ: &str = "members:read";
pub const SCOPE_MEMBERS_WRITE: &str = "members:write";
pub const SCOPE_TOKENS_READ: &str = "tokens:read";
pub const SCOPE_TOKENS_WRITE: &str = "tokens:write";
pub const SCOPE_SITE_READ: &str = "site:read";
pub const SCOPE_SCHEMA_READ: &str = "schema:read";
pub const SCOPE_SCHEMA_WRITE: &str = "schema:write";
pub const SCOPE_CONTENT_READ: &str = "content:read";
pub const SCOPE_CONTENT_WRITE: &str = "content:write";
pub const SCOPE_ASSETS_READ: &str = "assets:read";
pub const SCOPE_ASSETS_WRITE: &str = "assets:write";

fn extract_cookie_value(parts: &Parts, name: &str) -> Option<String> {
    let cookie_header = parts.headers.get("cookie")?.to_str().ok()?;
    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some((key, value)) = pair.split_once('=') {
            if key == name {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn extract_bearer_token(parts: &Parts) -> Option<String> {
    let auth_header = parts.headers.get("Authorization")?.to_str().ok()?;
    auth_header.strip_prefix("Bearer ").map(|s| s.to_string())
}

fn extract_csrf_token(parts: &Parts) -> Option<String> {
    let header = parts.headers.get("x-csrf-token")?.to_str().ok()?;
    Some(header.to_string())
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
}

#[derive(Debug, Clone)]
pub enum Principal {
    UserSession {
        user_id: String,
    },
    InstanceToken {
        token_id: String,
        scopes: BTreeSet<String>,
    },
    SiteToken {
        token_id: String,
        site_id: String,
        scopes: BTreeSet<String>,
    },
}

impl Principal {
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::UserSession { user_id } => Some(user_id),
            _ => None,
        }
    }

    pub fn bound_site_id(&self) -> Option<&str> {
        match self {
            Self::SiteToken { site_id, .. } => Some(site_id),
            _ => None,
        }
    }

    pub fn is_instance_token(&self) -> bool {
        matches!(self, Self::InstanceToken { .. })
    }

    pub fn is_site_token(&self) -> bool {
        matches!(self, Self::SiteToken { .. })
    }

    pub fn is_user_session(&self) -> bool {
        matches!(self, Self::UserSession { .. })
    }
}

#[derive(Debug, Clone)]
pub struct RequireInstanceToken(pub Principal);

impl<S> FromRequestParts<S> for RequireInstanceToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<AuthError>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let principal = Principal::from_request_parts(parts, state).await?;
        match principal {
            Principal::InstanceToken { .. } => Ok(Self(principal)),
            _ => Err(AuthError::instance_token_required()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequireSiteToken {
    pub site_id: String,
    pub scopes: BTreeSet<String>,
}

impl<S> FromRequestParts<S> for RequireSiteToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<AuthError>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let principal = Principal::from_request_parts(parts, state).await?;
        match principal {
            Principal::SiteToken {
                token_id: _,
                site_id,
                scopes,
            } => Ok(Self { site_id, scopes }),
            _ => Err(AuthError::site_token_required()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SiteOrUserPrincipal {
    SiteToken { site_id: String, scopes: BTreeSet<String> },
    UserSession { user_id: String },
}

impl<S> FromRequestParts<S> for SiteOrUserPrincipal
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<AuthError>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let principal = Principal::from_request_parts(parts, state).await?;
        match principal {
            Principal::SiteToken {
                token_id: _,
                site_id,
                scopes,
            } => Ok(Self::SiteToken { site_id, scopes }),
            Principal::UserSession { user_id } => Ok(Self::UserSession { user_id }),
            Principal::InstanceToken { .. } => Err(AuthError::instance_token_denied()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SiteContext {
    pub site_id: String,
}

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

    let claims = Claims {
        sub: user_id,
        exp: expiration,
    };

    encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
}

pub fn verify_token(token: &str, jwt_secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let data = decode::<Claims>(token, &DecodingKey::from_secret(jwt_secret.as_bytes()), &validation)?;

    Ok(data.claims)
}

pub fn parse_scopes(value: &str) -> BTreeSet<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn scopes_to_string(scopes: &[&str]) -> String {
    scopes.join(",")
}

pub fn default_instance_scopes() -> Vec<&'static str> {
    vec![
        SCOPE_SITES_READ,
        SCOPE_SITES_WRITE,
        SCOPE_SITES_DELETE,
        SCOPE_MEMBERS_READ,
        SCOPE_MEMBERS_WRITE,
        SCOPE_TOKENS_READ,
        SCOPE_TOKENS_WRITE,
    ]
}

pub fn default_site_scopes() -> Vec<&'static str> {
    vec![
        SCOPE_SITE_READ,
        SCOPE_SCHEMA_READ,
        SCOPE_SCHEMA_WRITE,
        SCOPE_CONTENT_READ,
        SCOPE_CONTENT_WRITE,
        SCOPE_ASSETS_READ,
        SCOPE_ASSETS_WRITE,
        SCOPE_TOKENS_READ,
        SCOPE_TOKENS_WRITE,
    ]
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<AuthError>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Principal::from_request_parts(parts, state).await? {
            Principal::UserSession { user_id } => Ok(Self { user_id }),
            _ => Err(AuthError::unauthorized("User session required")),
        }
    }
}

impl<S> FromRequestParts<S> for Principal
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<AuthError>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(token) = extract_bearer_token(parts) {
            if token.starts_with("cms_") {
                let repository = parts
                    .extensions
                    .get::<Repository>()
                    .ok_or_else(|| AuthError::unauthorized("Internal server error"))?;

                let config = parts
                    .extensions
                    .get::<Config>()
                    .ok_or_else(|| AuthError::unauthorized("Internal server error"))?;

                return verify_access_token(&token, repository, &config.hmac_secret).await;
            }

            let config = parts
                .extensions
                .get::<Config>()
                .ok_or_else(|| AuthError::unauthorized("Internal server error"))?;

            let claims = verify_token(&token, &config.jwt_secret)
                .map_err(|_| AuthError::unauthorized("Invalid or expired token"))?;

            Span::current().record("user_id", tracing::field::display(&claims.sub));

            return Ok(Self::UserSession { user_id: claims.sub });
        }

        if let Some(token) = extract_cookie_value(parts, "token") {
            let config = parts
                .extensions
                .get::<Config>()
                .ok_or_else(|| AuthError::unauthorized("Internal server error"))?;

            let claims = verify_token(&token, &config.jwt_secret)
                .map_err(|_| AuthError::unauthorized("Invalid or expired token"))?;

            Span::current().record("user_id", tracing::field::display(&claims.sub));

            return Ok(Self::UserSession { user_id: claims.sub });
        }

        Err(AuthError::unauthorized("Missing authentication"))
    }
}

pub(crate) async fn verify_access_token(
    token: &str,
    repository: &Repository,
    hmac_secret: &str,
) -> Result<Principal, (StatusCode, Json<AuthError>)> {
    let prefix: String = token.chars().take(24).collect();

    let keys = repository
        .access_token
        .find_by_prefix(&prefix)
        .await
        .map_err(|_| AuthError::unauthorized("Internal server error"))?;

    let token_hmac = compute_key_hmac(token, hmac_secret);

    for (token_id, kind, site_id, stored_hash, stored_hmac, expires_at, revoked_at, scopes) in keys {
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

        if let Some(exp) = expires_at {
            if let Ok(expiry) = chrono::NaiveDateTime::parse_from_str(&exp, "%Y-%m-%d %H:%M:%S") {
                if expiry < chrono::Utc::now().naive_utc() {
                    return Err(AuthError::unauthorized("Access token has expired"));
                }
            }
        }

        let _ = repository.access_token.update_last_used(&token_id).await;

        let kind = AccessTokenKind::from_str(&kind).map_err(|_| AuthError::unauthorized("Invalid access token"))?;
        let scopes = parse_scopes(&scopes);

        return match kind {
            AccessTokenKind::Instance => Ok(Principal::InstanceToken { token_id, scopes }),
            AccessTokenKind::Site => {
                let site_id = site_id.ok_or_else(|| AuthError::unauthorized("Site token missing site binding"))?;
                Span::current().record("site_id", tracing::field::display(&site_id));
                Ok(Principal::SiteToken {
                    token_id,
                    site_id,
                    scopes,
                })
            }
        };
    }

    Err(AuthError::unauthorized("Invalid access token"))
}

pub fn verify_csrf(parts: &Parts, config: &Config) -> Result<(), (StatusCode, String)> {
    if !config.cookie_secure {
        return Ok(());
    }

    let csrf_cookie =
        extract_cookie_value(parts, "csrf").ok_or((StatusCode::FORBIDDEN, "Missing CSRF cookie".to_string()))?;

    let csrf_header = extract_csrf_token(parts).ok_or((StatusCode::FORBIDDEN, "Missing CSRF token".to_string()))?;

    if csrf_cookie != csrf_header {
        return Err((StatusCode::FORBIDDEN, "CSRF token mismatch".to_string()));
    }

    Ok(())
}

pub async fn require_admin_scope(
    principal: &Principal,
    repository: &Repository,
    site_id: Option<&str>,
    scope: &str,
) -> Result<(), (StatusCode, Json<AuthError>)> {
    match principal {
        Principal::InstanceToken { scopes, .. } => {
            if scopes.contains(scope) {
                Ok(())
            } else {
                Err(AuthError::insufficient_scope(scope))
            }
        }
        Principal::UserSession { user_id } => match scope {
            SCOPE_SITES_READ | SCOPE_SITES_WRITE => Ok(()),
            SCOPE_SITES_DELETE => {
                let site_id = site_id.ok_or_else(|| AuthError::forbidden("Site id is required"))?;
                check_site_access_repo(repository, user_id, site_id, "owner").await
            }
            SCOPE_MEMBERS_READ => {
                let site_id = site_id.ok_or_else(|| AuthError::forbidden("Site id is required"))?;
                check_site_access_repo(repository, user_id, site_id, "viewer").await
            }
            SCOPE_MEMBERS_WRITE | SCOPE_TOKENS_READ | SCOPE_TOKENS_WRITE => {
                let site_id = site_id.ok_or_else(|| AuthError::forbidden("Site id is required"))?;
                check_site_access_repo(repository, user_id, site_id, "admin").await
            }
            _ => Err(AuthError::forbidden("Unsupported admin scope")),
        },
        Principal::SiteToken { .. } => Err(AuthError::site_token_denied()),
    }
}

pub async fn resolve_site_context(
    principal: &Principal,
    repository: &Repository,
    explicit_site_id: Option<&str>,
) -> Result<SiteContext, (StatusCode, Json<AuthError>)> {
    match principal {
        Principal::SiteToken { site_id, .. } => {
            if let Some(explicit) = explicit_site_id {
                if explicit != site_id {
                    return Err(AuthError::forbidden("Site token does not have access to this site"));
                }
            }
            Ok(SiteContext {
                site_id: site_id.clone(),
            })
        }
        Principal::UserSession { user_id } => {
            let site_id = explicit_site_id.ok_or_else(|| AuthError::forbidden("Missing site context"))?;
            check_site_access_repo(repository, user_id, site_id, "viewer").await?;
            Ok(SiteContext {
                site_id: site_id.to_string(),
            })
        }
        Principal::InstanceToken { .. } => Err(AuthError::instance_token_denied()),
    }
}

pub async fn require_site_scope(
    principal: &Principal,
    repository: &Repository,
    explicit_site_id: Option<&str>,
    scope: &str,
    jwt_min_role: &str,
) -> Result<SiteContext, (StatusCode, Json<AuthError>)> {
    let site = resolve_site_context(principal, repository, explicit_site_id).await?;

    match principal {
        Principal::SiteToken { scopes, .. } => {
            if scopes.contains(scope) {
                Ok(site)
            } else {
                Err(AuthError::insufficient_scope(scope))
            }
        }
        Principal::UserSession { user_id } => {
            check_site_access_repo(repository, user_id, &site.site_id, jwt_min_role).await?;
            Ok(site)
        }
        Principal::InstanceToken { .. } => Err(AuthError::instance_token_denied()),
    }
}

pub fn site_context_from_headers(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(HEADER_SITE_ID)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

pub fn extract_user_id(principal: &Principal) -> Option<&str> {
    principal.user_id()
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
    fn test_parse_scopes() {
        let scopes = parse_scopes("site:read, content:write,content:write");
        assert!(scopes.contains("site:read"));
        assert!(scopes.contains("content:write"));
        assert_eq!(scopes.len(), 2);
    }

    #[test]
    fn test_compute_key_hmac_deterministic() {
        let key = "cms_site_abcdefgh_1234567890123456789012";
        let secret = "test-hmac-secret";
        let h1 = compute_key_hmac(key, secret);
        let h2 = compute_key_hmac(key, secret);
        assert_eq!(h1, h2);
    }
}
