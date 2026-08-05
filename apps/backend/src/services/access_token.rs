use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::middleware::auth::compute_key_hmac;
use crate::models::access_token::{
    AccessTokenResponse, AccessTokenView, CreatePersonalAccessToken, PersonalAccessTokenResponse,
    PersonalAccessTokenView, TokenScopes, decode_scopes, encode_scopes,
};
use crate::repository::traits::{AccessTokenRepository, NewAccessToken, NewPersonalToken};

const SITE_TOKEN_PREFIX: &str = "vcms_site_";

#[derive(Clone)]
pub struct AccessTokenService {
    access_token_repo: Arc<dyn AccessTokenRepository>,
    hmac_secret: String,
}

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Token not found")]
    NotFound,

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
            TokenError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))),
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

    fn build_personal_token() -> String {
        format!("vcms_pat_{}", Uuid::new_v4().simple())
    }

    pub async fn list_site_tokens(&self, site_id: &str) -> Result<Vec<AccessTokenView>, TokenError> {
        self.access_token_repo
            .list(site_id)
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))?
            .into_iter()
            .map(|token| {
                Ok(AccessTokenView {
                    id: token.id,
                    site_id: token.site_id,
                    name: token.name,
                    token_prefix: token.token_prefix,
                    scopes: decode_scopes(&token.scopes_json)
                        .map_err(|error| TokenError::DatabaseError(error.to_string()))?,
                    created_by_user_id: token.created_by_user_id,
                    last_used_at: token.last_used_at,
                    created_at: token.created_at,
                    expires_at: token.expires_at,
                    revoked_at: token.revoked_at,
                })
            })
            .collect()
    }

    pub async fn create_site_token(
        &self,
        site_id: &str,
        name: String,
        scopes: impl Into<TokenScopes>,
        expires_at: Option<&str>,
        created_by: Option<&str>,
    ) -> Result<AccessTokenResponse, TokenError> {
        debug!("Creating scoped site token: site_id={}", site_id);
        let scopes = scopes.into();

        let name = name.trim();
        if name.is_empty() {
            return Err(TokenError::NameRequired);
        }

        let raw_token = Self::build_token();
        let prefix: String = raw_token.chars().take(24).collect();
        let token_hmac = compute_key_hmac(&raw_token, &self.hmac_secret);
        let id = Uuid::now_v7().to_string();
        let scopes_json = encode_scopes(&scopes).map_err(|e| TokenError::DatabaseError(e.to_string()))?;

        self.access_token_repo
            .create(NewAccessToken {
                id: &id,
                site_id,
                name,
                token_prefix: &prefix,
                token_hmac: &token_hmac,
                scopes_json: &scopes_json,
                created_by_user_id: created_by,
                expires_at,
            })
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
            scopes,
            created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            expires_at: expires_at.map(str::to_string),
        })
    }

    pub async fn list_personal_tokens(&self, user_id: &str) -> Result<Vec<PersonalAccessTokenView>, TokenError> {
        self.access_token_repo
            .list_personal(user_id)
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))?
            .into_iter()
            .map(|t| {
                Ok(PersonalAccessTokenView {
                    id: t.id,
                    name: t.name,
                    token_prefix: t.token_prefix,
                    scopes: decode_scopes(&t.scopes_json).map_err(|e| TokenError::DatabaseError(e.to_string()))?,
                    last_used_at: t.last_used_at,
                    created_at: t.created_at,
                    expires_at: t.expires_at,
                    revoked_at: t.revoked_at,
                })
            })
            .collect()
    }

    pub async fn create_personal_token(
        &self,
        user_id: &str,
        payload: CreatePersonalAccessToken,
    ) -> Result<PersonalAccessTokenResponse, TokenError> {
        let name = payload.name.trim();
        if name.is_empty() {
            return Err(TokenError::NameRequired);
        }
        let raw = Self::build_personal_token();
        let prefix: String = raw.chars().take(24).collect();
        let id = Uuid::now_v7().to_string();
        let token_hmac = compute_key_hmac(&raw, &self.hmac_secret);
        let scopes_json = encode_scopes(&payload.scopes).map_err(|e| TokenError::DatabaseError(e.to_string()))?;
        self.access_token_repo
            .create_personal(NewPersonalToken {
                id: &id,
                user_id,
                name,
                token_hmac: &token_hmac,
                token_prefix: &prefix,
                scopes_json: &scopes_json,
                expires_at: payload.expires_at.as_deref(),
            })
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))?;
        Ok(PersonalAccessTokenResponse {
            token_info: PersonalAccessTokenView {
                id,
                name: name.into(),
                token_prefix: prefix,
                scopes: payload.scopes,
                last_used_at: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                expires_at: payload.expires_at,
                revoked_at: None,
            },
            token: raw,
        })
    }

    pub async fn revoke_personal_token(&self, id: &str, user_id: &str) -> Result<u64, TokenError> {
        self.access_token_repo
            .revoke_personal(id, user_id)
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))
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
    use crate::models::access_token::TokenScope;
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
                [TokenScope::SiteRead, TokenScope::ContentRead]
                    .into_iter()
                    .collect::<TokenScopes>(),
                None,
                None,
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.name, "Test Token");
        assert_eq!(response.site_id, "site-123");
        assert!(
            response
                .scopes
                .contains(&crate::models::access_token::TokenScope::ContentRead)
        );
        assert!(response.token.starts_with(SITE_TOKEN_PREFIX));
    }

    #[tokio::test]
    async fn test_create_site_token_empty_name() {
        let service = test_service(test_repo());

        let result = service
            .create_site_token(
                "site-123",
                "   ".to_string(),
                [TokenScope::SiteRead].into_iter().collect::<TokenScopes>(),
                None,
                None,
            )
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
                [TokenScope::SiteSettingsWrite, TokenScope::ContentWrite]
                    .into_iter()
                    .collect::<TokenScopes>(),
                None,
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
