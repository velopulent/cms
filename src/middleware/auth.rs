use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use sqlx::SqlitePool;

use crate::config::Config;
use crate::models::user::Claims;

// --- Auth Context ---

pub enum AuthContext {
    Jwt { user_id: String },
    ApiKey { site_id: String },
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
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(unauthorized_error("Missing authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(unauthorized_error("Invalid authorization format"))?;

        let config = parts
            .extensions
            .get::<Config>()
            .ok_or(unauthorized_error("Internal server error"))?;

        let claims = verify_token(token, &config.jwt_secret)
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
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(unauthorized_error("Missing authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(unauthorized_error("Invalid authorization format"))?;

        // Try API key first (prefixed with "cms_")
        if token.starts_with("cms_") {
            let pool = parts
                .extensions
                .get::<SqlitePool>()
                .ok_or(unauthorized_error("Internal server error"))?;

            return verify_api_key(token, pool).await;
        }

        // Otherwise try JWT
        let config = parts
            .extensions
            .get::<Config>()
            .ok_or(unauthorized_error("Internal server error"))?;

        let claims = verify_token(token, &config.jwt_secret)
            .map_err(|_| unauthorized_error("Invalid or expired token"))?;

        Ok(AuthContext::Jwt {
            user_id: claims.sub,
        })
    }
}

async fn verify_api_key(
    token: &str,
    pool: &SqlitePool,
) -> Result<AuthContext, (StatusCode, String)> {
    // key_prefix is the first 16 chars of the raw key (e.g., "cms_a1b2c3d4e")
    let prefix: String = token.chars().take(16).collect();

    let keys = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT id, site_id, key_hash, expires_at FROM api_keys WHERE key_prefix = ?",
    )
    .bind(&prefix)
    .fetch_all(pool)
    .await
    .map_err(|_| unauthorized_error("Internal server error"))?;

    for (key_id, site_id, stored_hash, expires_at) in keys {
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
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = datetime('now') WHERE id = ?")
            .bind(&key_id)
            .execute(pool)
            .await;

        return Ok(AuthContext::ApiKey { site_id });
    }

    Err(unauthorized_error("Invalid API key"))
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

    let result: Option<(String,)> = sqlx::query_as(
        "SELECT sm.role FROM site_members sm WHERE sm.user_id = ? AND sm.site_id = ?",
    )
    .bind(user_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            serde_json::json!({"error": e.to_string()}),
        )
    })?;

    match result {
        Some((role,)) if role_order(&role) >= min_level => Ok(()),
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
