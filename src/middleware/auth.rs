use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use sqlx::SqlitePool;

use crate::config::Config;
use crate::models::user::Claims;
use crate::repository::api_key as api_key_repo;
use crate::repository::user as user_repo;

// --- Cookie helpers ---

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
    // Prefer cookie over Authorization header
    if let Some(token) = extract_cookie_value(parts, "token") {
        return Some(token);
    }
    // Fallback to Authorization: Bearer header
    let auth_header = parts.headers.get("Authorization")?.to_str().ok()?;
    auth_header.strip_prefix("Bearer ").map(|s| s.to_string())
}

// --- Auth Context ---

pub enum AuthContext {
    Jwt { user_id: String },
    ApiKey { site_id: String, permissions: String },
}

// --- JWT-only extractor (dashboard endpoints) ---

pub struct AuthenticatedUser {
    pub user_id: String,
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

    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(jwt_secret.as_bytes()),
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

// --- AuthenticatedUser: JWT-only extractor ---

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

        Ok(AuthenticatedUser {
            user_id: claims.sub,
        })
    }
}

// --- AuthContext: Dual JWT + API key extractor ---

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Check Authorization header first for API key (external clients)
        if let Some(auth_header) = parts.headers.get("Authorization").and_then(|v| v.to_str().ok()) {
            if let Some(token) = auth_header.strip_prefix("Bearer ") {
                // API key auth (prefixed with "cms_")
                if token.starts_with("cms_") {
                    let pool = parts
                        .extensions
                        .get::<SqlitePool>()
                        .ok_or(unauthorized_error("Internal server error"))?;

                    return verify_api_key(token, pool).await;
                }

                // JWT from Authorization header
                let config = parts
                    .extensions
                    .get::<Config>()
                    .ok_or(unauthorized_error("Internal server error"))?;

                let claims = verify_token(token, &config.jwt_secret)
                    .map_err(|_| unauthorized_error("Invalid or expired token"))?;

                return Ok(AuthContext::Jwt {
                    user_id: claims.sub,
                });
            }
        }

        // Fallback: JWT from cookie
        if let Some(token) = extract_cookie_value(parts, "token") {
            let config = parts
                .extensions
                .get::<Config>()
                .ok_or(unauthorized_error("Internal server error"))?;

            let claims = verify_token(&token, &config.jwt_secret)
                .map_err(|_| unauthorized_error("Invalid or expired token"))?;

            return Ok(AuthContext::Jwt {
                user_id: claims.sub,
            });
        }

        Err(unauthorized_error("Missing authentication"))
    }
}

pub(crate) async fn verify_api_key(
    token: &str,
    pool: &SqlitePool,
) -> Result<AuthContext, (StatusCode, String)> {
    // key_prefix is the first 16 chars of the raw key (e.g., "cms_a1b2c3d4e")
    let prefix: String = token.chars().take(16).collect();

    let keys = api_key_repo::find_by_prefix(pool, &prefix)
        .await
        .map_err(|_| unauthorized_error("Internal server error"))?;

    for (key_id, site_id, stored_hash, expires_at, permissions) in keys {
        if !bcrypt::verify(token, &stored_hash).unwrap_or(false) {
            continue;
        }

        // Check expiry
        if let Some(exp) = expires_at {
            if let Ok(expiry) = chrono::NaiveDateTime::parse_from_str(&exp, "%Y-%m-%d %H:%M:%S") {
                if expiry < chrono::Utc::now().naive_utc() {
                    return Err(unauthorized_error("API key has expired"));
                }
            }
        }

        // Update last_used_at (fire and forget)
        api_key_repo::update_last_used(pool, &key_id).await;

        return Ok(AuthContext::ApiKey { site_id, permissions });
    }

    Err(unauthorized_error("Invalid API key"))
}

// --- Unified access checks ---

pub async fn check_read_access(
    auth: &AuthContext,
    pool: &SqlitePool,
    site_id: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
    match auth {
        AuthContext::Jwt { user_id } => {
            check_site_access(pool, user_id, site_id, "viewer").await
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

pub async fn check_write_access(
    auth: &AuthContext,
    pool: &SqlitePool,
    site_id: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
    match auth {
        AuthContext::Jwt { user_id } => {
            check_site_access(pool, user_id, site_id, "editor").await
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

// --- Site access checks ---

pub async fn check_site_access(
    pool: &SqlitePool,
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

    let role = user_repo::get_role(pool, user_id, site_id).await.map_err(|e| {
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
