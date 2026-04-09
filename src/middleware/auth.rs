use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use hmac::{Hmac, Mac};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation, decode, encode};
use sha2::Sha256;
use tracing::Span;

use crate::config::Config;
use crate::models::user::Claims;
use crate::repository::Repository;

type HmacSha256 = Hmac<Sha256>;

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

fn extract_jwt_token(parts: &Parts) -> Option<String> {
    if let Some(token) = extract_cookie_value(parts, "token") {
        return Some(token);
    }
    let auth_header = parts.headers.get("Authorization")?.to_str().ok()?;
    auth_header.strip_prefix("Bearer ").map(|s| s.to_string())
}

fn extract_csrf_token(parts: &Parts) -> Option<String> {
    let header = parts.headers.get("x-csrf-token")?.to_str().ok()?;
    Some(header.to_string())
}

#[derive(Debug)]
pub enum AuthContext {
    Jwt { user_id: String },
    ApiKey { site_id: String, permissions: String },
}

#[derive(Debug)]
pub struct AuthenticatedUser {
    pub user_id: String,
}

pub fn compute_key_hmac(key: &str, hmac_secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(key.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn create_token(
    user_id: String,
    jwt_secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
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

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )?;

    Ok(data.claims)
}

fn unauthorized_error(msg: &str) -> (StatusCode, String) {
    (
        StatusCode::UNAUTHORIZED,
        serde_json::json!({"error": msg}).to_string(),
    )
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = extract_jwt_token(parts)
            .ok_or(unauthorized_error("Missing authentication"))?;

        let config = parts
            .extensions
            .get::<Config>()
            .ok_or(unauthorized_error("Internal server error"))?;

        let claims = verify_token(&token, &config.jwt_secret)
            .map_err(|_| unauthorized_error("Invalid or expired token"))?;

        Span::current().record("user_id", tracing::field::display(&claims.sub));

        Ok(AuthenticatedUser {
            user_id: claims.sub,
        })
    }
}

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try Authorization header first
        if let Some(auth_header) = parts.headers.get("Authorization").and_then(|v| v.to_str().ok()) {
            if let Some(token) = auth_header.strip_prefix("Bearer ") {
                if token.starts_with("cms_") {
                    let repository = parts
                        .extensions
                        .get::<Repository>()
                        .ok_or(unauthorized_error("Internal server error"))?;

                    let config = parts
                        .extensions
                        .get::<Config>()
                        .ok_or(unauthorized_error("Internal server error"))?;

                    return verify_api_key(token, repository, &config.hmac_secret).await;
                }

                let config = parts
                    .extensions
                    .get::<Config>()
                    .ok_or(unauthorized_error("Internal server error"))?;

                let claims = verify_token(token, &config.jwt_secret)
                    .map_err(|_| unauthorized_error("Invalid or expired token"))?;

                Span::current().record("user_id", tracing::field::display(&claims.sub));

                return Ok(AuthContext::Jwt {
                    user_id: claims.sub,
                });
            }
        }

        // Try cookie
        if let Some(token) = extract_cookie_value(parts, "token") {
            let config = parts
                .extensions
                .get::<Config>()
                .ok_or(unauthorized_error("Internal server error"))?;

        let claims = verify_token(&token, &config.jwt_secret)
            .map_err(|_| unauthorized_error("Invalid or expired token"))?;

        Span::current().record("user_id", tracing::field::display(&claims.sub));

        return Ok(AuthContext::Jwt {
            user_id: claims.sub,
        });
        }

        Err(unauthorized_error("Missing authentication"))
    }
}

pub(crate) async fn verify_api_key(
    token: &str,
    repository: &Repository,
    hmac_secret: &str,
) -> Result<AuthContext, (StatusCode, String)> {
    let prefix: String = token.chars().take(16).collect();

    let keys = repository.api_key.find_by_prefix(&prefix)
        .await
        .map_err(|_| unauthorized_error("Internal server error"))?;

    let token_hmac = compute_key_hmac(token, hmac_secret);

    for (key_id, site_id, stored_hash, stored_hmac, expires_at, permissions) in keys {
        // Fast path: compare HMAC-SHA256 (microseconds) instead of bcrypt (~100ms)
        if let Some(ref stored) = stored_hmac {
            if stored != &token_hmac {
                continue;
            }
        } else {
            // Legacy: fall back to bcrypt for keys created before HMAC was added
            if !bcrypt::verify(token, &stored_hash).unwrap_or(false) {
                continue;
            }
        }

        if let Some(exp) = expires_at {
            if let Ok(expiry) = chrono::NaiveDateTime::parse_from_str(&exp, "%Y-%m-%d %H:%M:%S") {
                if expiry < chrono::Utc::now().naive_utc() {
                    return Err(unauthorized_error("API key has expired"));
                }
            }
        }

        let _ = repository.api_key.update_last_used(&key_id).await;

        Span::current().record("site_id", tracing::field::display(&site_id));

        return Ok(AuthContext::ApiKey { site_id, permissions });
    }

    Err(unauthorized_error("Invalid API key"))
}

pub fn verify_csrf(parts: &Parts, config: &Config) -> Result<(), (StatusCode, String)> {
    if !config.cookie_secure {
        return Ok(());
    }

    let csrf_cookie = extract_cookie_value(parts, "csrf")
        .ok_or((StatusCode::FORBIDDEN, "Missing CSRF cookie".to_string()))?;

    let csrf_header = extract_csrf_token(parts)
        .ok_or((StatusCode::FORBIDDEN, "Missing CSRF token".to_string()))?;

    if csrf_cookie != csrf_header {
        return Err((StatusCode::FORBIDDEN, "CSRF token mismatch".to_string()));
    }

    Ok(())
}

pub async fn check_read_access_repo(
    auth: &AuthContext,
    repository: &Repository,
    site_id: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
    match auth {
        AuthContext::Jwt { user_id } => {
            check_site_access_repo(repository, user_id, site_id, "viewer").await
        }
        AuthContext::ApiKey { site_id: key_site_id, .. } => {
            if key_site_id == site_id {
                Ok(())
            } else {
                Err((
                    StatusCode::FORBIDDEN,
                    serde_json::json!({"error": "API key does not have access to this site"}),
                ))
            }
        }
    }
}

pub async fn check_write_access_repo(
    auth: &AuthContext,
    repository: &Repository,
    site_id: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
    match auth {
        AuthContext::Jwt { user_id } => {
            check_site_access_repo(repository, user_id, site_id, "editor").await
        }
        AuthContext::ApiKey { site_id: key_site_id, permissions } => {
            if key_site_id != site_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    serde_json::json!({"error": "API key does not have access to this site"}),
                ));
            }
            if permissions != "write" {
                return Err((
                    StatusCode::FORBIDDEN,
                    serde_json::json!({"error": "API key does not have write permissions"}),
                ));
            }
            Ok(())
        }
    }
}

pub fn extract_user_id(auth: &AuthContext) -> Option<&str> {
    match auth {
        AuthContext::Jwt { user_id } => Some(user_id),
        AuthContext::ApiKey { .. } => None,
    }
}

pub async fn check_site_access_repo(
    repository: &Repository,
    user_id: &str,
    site_id: &str,
    min_role: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
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
            serde_json::json!({"error": e.to_string()}),
        )
    })?;

    match role {
        Some(r) if role_order(&r) >= min_level => Ok(()),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            serde_json::json!({"error": "Insufficient permissions"}),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "Site not found"}),
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
    fn test_verify_token_with_wrong_secret() {
        let user_id = "test-user-123";
        let secret = "test-jwt-secret";
        let wrong_secret = "wrong-secret";

        let token = create_token(user_id.to_string(), secret).expect("Token creation should succeed");
        let result = verify_token(&token, wrong_secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_with_malformed_token() {
        let secret = "test-jwt-secret";
        let result = verify_token("not.a.valid.token", secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_with_empty_token() {
        let secret = "test-jwt-secret";
        let result = verify_token("", secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_user_id_from_jwt_context() {
        let auth = AuthContext::Jwt {
            user_id: "user123".to_string(),
        };
        assert_eq!(extract_user_id(&auth), Some("user123"));
    }

    #[test]
    fn test_extract_user_id_from_api_key_context() {
        let auth = AuthContext::ApiKey {
            site_id: "site123".to_string(),
            permissions: "read".to_string(),
        };
        assert_eq!(extract_user_id(&auth), None);
    }

    #[test]
    fn test_compute_key_hmac_deterministic() {
        let key = "cms_abcdefgh_1234567890123456789012";
        let secret = "test-hmac-secret";
        let h1 = compute_key_hmac(key, secret);
        let h2 = compute_key_hmac(key, secret);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_key_hmac_different_keys_produce_different_hmacs() {
        let secret = "test-hmac-secret";
        let h1 = compute_key_hmac("key1", secret);
        let h2 = compute_key_hmac("key2", secret);
        assert_ne!(h1, h2);
    }
}