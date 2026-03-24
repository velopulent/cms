use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

use crate::models::user::Claims;

const JWT_SECRET: &str = "cms-jwt-secret-change-in-production";

pub struct AuthenticatedUser {
    pub user_id: String,
}

pub fn create_token(user_id: String) -> Result<String, jsonwebtoken::errors::Error> {
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
        &jsonwebtoken::EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
}

pub fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &validation,
    )?;

    Ok(data.claims)
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": "Missing authorization header"}).to_string(),
            ))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or((
                StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": "Invalid authorization format"}).to_string(),
            ))?;

        let claims = verify_token(token).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": "Invalid or expired token"}).to_string(),
            )
        })?;

        Ok(AuthenticatedUser {
            user_id: claims.sub,
        })
    }
}

pub async fn check_site_access(
    pool: &sqlx::SqlitePool,
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
