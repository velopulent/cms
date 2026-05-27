use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use bcrypt::{DEFAULT_COST, hash};
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::middleware::auth::compute_key_hmac;
use crate::models::access_token::{AccessToken, AccessTokenPermission, AccessTokenResponse};
use crate::repository::traits::AccessTokenRepository;

const SITE_TOKEN_PREFIX: &str = "cms_site_";

#[derive(Clone)]
pub struct AccessTokenService {
    access_token_repo: Arc<dyn AccessTokenRepository>,
    hmac_secret: String,
}

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Token not found")]
    NotFound,

    #[error("Hash error: {0}")]
    HashError(String),

    #[error("Name is required")]
    NameRequired,

    #[error("Database error: {0}")]
    DatabaseError(String),
}

impl TokenError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            TokenError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Token not found"}))),
            TokenError::NameRequired => (StatusCode::BAD_REQUEST, Json(json!({"error": "Name is required"}))),
            TokenError::HashError(msg) | TokenError::DatabaseError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
            }
        };
        (status, body).into_response()
    }
}

impl AccessTokenService {
    pub fn new(access_token_repo: Arc<dyn AccessTokenRepository>, hmac_secret: String) -> Self {
        Self {
            access_token_repo,
            hmac_secret,
        }
    }

    fn build_token() -> String {
        let random_chars = Uuid::new_v4().to_string().replace('-', "");
        format!("{}{}", SITE_TOKEN_PREFIX, random_chars)
    }

    pub async fn list_site_tokens(&self, site_id: &str) -> Result<Vec<AccessToken>, TokenError> {
        self.access_token_repo
            .list(site_id)
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))
    }

    pub async fn create_site_token(
        &self,
        site_id: &str,
        name: String,
        permission: AccessTokenPermission,
        created_by: Option<&str>,
    ) -> Result<AccessTokenResponse, TokenError> {
        debug!(
            "Creating site token: site_id={}, permission={}",
            site_id, permission
        );

        let name = name.trim();
        if name.is_empty() {
            return Err(TokenError::NameRequired);
        }

        let raw_token = Self::build_token();
        let prefix: String = raw_token.chars().take(24).collect();
        let token_hash = hash(&raw_token, DEFAULT_COST).map_err(|e| TokenError::HashError(e.to_string()))?;
        let token_hmac = compute_key_hmac(&raw_token, &self.hmac_secret);
        let id = Uuid::now_v7().to_string();
        let permission_str = permission.as_str();

        self.access_token_repo
            .create(
                &id,
                site_id,
                name,
                &token_hash,
                &prefix,
                &token_hmac,
                permission_str,
                created_by,
            )
            .await
            .map_err(|e| {
                error!("Failed to create site token: site_id={}, error={}", site_id, e);
                TokenError::DatabaseError(e.to_string())
            })?;

        Ok(AccessTokenResponse {
            id,
            site_id: site_id.to_string(),
            name: name.to_string(),
            token: raw_token,
            token_prefix: prefix,
            permission: permission_str.to_string(),
            created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }

    pub async fn delete_site_token(&self, token_id: &str, site_id: &str) -> Result<u64, TokenError> {
        debug!("Deleting site token: token_id={}, site_id={}", token_id, site_id);

        match self.access_token_repo.delete(token_id, site_id).await {
            Ok(deleted_count) => {
                info!(
                    "Site token deleted successfully: token_id={}, deleted_count={}",
                    token_id, deleted_count
                );
                Ok(deleted_count)
            }
            Err(e) => {
                error!(
                    "Failed to delete site token: token_id={}, site_id={}, error={}",
                    token_id, site_id, e
                );
                Err(TokenError::DatabaseError(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::InMemoryAccessTokenRepository;

    fn test_repo() -> Arc<InMemoryAccessTokenRepository> {
        Arc::new(InMemoryAccessTokenRepository::new())
    }

    fn test_service(repo: Arc<InMemoryAccessTokenRepository>) -> AccessTokenService {
        AccessTokenService::new(repo, "hmac-secret-key".to_string())
    }

    #[tokio::test]
    async fn test_create_site_token_success() {
        let service = test_service(test_repo());

        let result = service
            .create_site_token(
                "site-123",
                "Test Token".to_string(),
                AccessTokenPermission::Read,
                None,
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.name, "Test Token");
        assert_eq!(response.site_id, "site-123");
        assert_eq!(response.permission, "read");
        assert!(response.token.starts_with(SITE_TOKEN_PREFIX));
    }

    #[tokio::test]
    async fn test_create_site_token_empty_name() {
        let service = test_service(test_repo());

        let result = service
            .create_site_token("site-123", "   ".to_string(), AccessTokenPermission::Read, None)
            .await;

        assert!(matches!(result, Err(TokenError::NameRequired)));
    }

    #[tokio::test]
    async fn test_list_site_tokens() {
        let service = test_service(test_repo());

        let result = service.list_site_tokens("site-123").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_delete_site_token() {
        let service = test_service(test_repo());

        let create_result = service
            .create_site_token(
                "site-123",
                "To Delete".to_string(),
                AccessTokenPermission::Write,
                None,
            )
            .await
            .unwrap();

        let result = service.delete_site_token(&create_result.id, "site-123").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_build_token_format() {
        let token = AccessTokenService::build_token();
        assert!(token.starts_with(SITE_TOKEN_PREFIX));
        assert_eq!(token.len(), SITE_TOKEN_PREFIX.len() + 32);
    }
}
