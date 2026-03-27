use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bcrypt::{DEFAULT_COST, hash};
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::middleware::auth::{AuthenticatedUser, check_site_access};
use crate::models::api_key::{ApiKey, ApiKeyResponse, CreateApiKey};

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/api-keys",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 200, description = "List of API keys", body = Vec<ApiKey>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "api-keys"
)]
pub async fn list_api_keys(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query_as::<_, ApiKey>(
        "SELECT id, site_id, name, key_hash, key_prefix, permissions, last_used_at, created_at, expires_at
         FROM api_keys WHERE site_id = ? ORDER BY created_at DESC",
    )
    .bind(&site_id)
    .fetch_all(&pool)
    .await;

    match result {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/api-keys",
    params(("site_id" = String, Path, description = "Site ID")),
    request_body = CreateApiKey,
    responses(
        (status = 201, description = "API key created (key shown only once)", body = ApiKeyResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "api-keys"
)]
pub async fn create_api_key(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateApiKey>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Name is required"})),
        )
            .into_response();
    }

    // Generate key: cms_{8chars}_{24chars} = 35 chars total
    // Use UUIDs for randomness (no rand dependency needed)
    let random_chars = Uuid::new_v4().to_string().replace('-', "");
    let segment_a: String = random_chars.chars().take(8).collect();
    let segment_b: String = random_chars.chars().skip(8).take(24).collect();
    let raw_key = format!("cms_{}_{}", segment_a, segment_b);

    // Prefix is first 16 chars for lookup
    let prefix: String = raw_key.chars().take(16).collect();

    let key_hash = match hash(&raw_key, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Hash error: {}", e)})),
            )
                .into_response()
        }
    };

    let id = Uuid::now_v7().to_string();

    let result = sqlx::query(
        "INSERT INTO api_keys (id, site_id, name, key_hash, key_prefix) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&site_id)
    .bind(&payload.name)
    .bind(&key_hash)
    .bind(&prefix)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

            (
                StatusCode::CREATED,
                Json(ApiKeyResponse {
                    id: id.clone(),
                    site_id: site_id.clone(),
                    name: payload.name,
                    key: raw_key,
                    key_prefix: prefix,
                    permissions: "read".to_string(),
                    created_at: now,
                }),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/api-keys/{key_id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("key_id" = String, Path, description = "API Key ID"),
    ),
    responses(
        (status = 204, description = "API key deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "API key not found"),
    ),
    security(("bearer" = [])),
    tag = "api-keys"
)]
pub async fn delete_api_key(
    auth: AuthenticatedUser,
    Path((site_id, key_id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query("DELETE FROM api_keys WHERE id = ? AND site_id = ?")
        .bind(&key_id)
        .bind(&site_id)
        .execute(&pool)
        .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "API key not found"})),
        )
            .into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}
