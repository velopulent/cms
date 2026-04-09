use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bcrypt::{DEFAULT_COST, hash};
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use crate::middleware::auth::{AuthenticatedUser, check_site_access_repo, compute_key_hmac};
use crate::models::api_key::{ApiKeyResponse, CreateApiKey};
use crate::repository::Repository;

#[instrument(skip(repository, auth))]
pub async fn list_api_keys(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    match repository.api_key.list(&site_id).await {
        Ok(items) => (StatusCode::OK, Json(items)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[instrument(skip(repository, config, auth, payload))]
pub async fn create_api_key(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(config): Extension<crate::config::Config>,
    Json(payload): Json<CreateApiKey>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Name is required"})),
        )
            .into_response();
    }

    let permissions = match payload.permissions.as_deref() {
        Some("read") | None => "read",
        Some("write") => "write",
        Some(other) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Invalid permissions '{}'. Must be 'read' or 'write'", other)})),
            )
                .into_response();
        }
    };

    let random_chars = Uuid::new_v4().to_string().replace('-', "");
    let segment_a: String = random_chars.chars().take(8).collect();
    let segment_b: String = random_chars.chars().skip(8).take(24).collect();
    let raw_key = format!("cms_{}_{}", segment_a, segment_b);

    let prefix: String = raw_key.chars().take(16).collect();

    let key_hash = match hash(&raw_key, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Hash error: {}", e)})),
            )
                .into_response();
        }
    };

    let key_hmac = compute_key_hmac(&raw_key, &config.hmac_secret);

    let id = Uuid::now_v7().to_string();

    match repository.api_key.create(&id, &site_id, &payload.name, &key_hash, &prefix, &key_hmac, permissions).await {
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
                    permissions: permissions.to_string(),
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

#[instrument(skip(repository, auth))]
pub async fn delete_api_key(
    auth: AuthenticatedUser,
    Path((site_id, key_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    match repository.api_key.delete(&key_id, &site_id).await {
        Ok(0) => (
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