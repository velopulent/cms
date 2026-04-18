use std::sync::Arc;

use axum::{Json, http::StatusCode, response::IntoResponse};
use bcrypt::{DEFAULT_COST, hash};
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::middleware::auth::{compute_key_hmac, default_instance_scopes, default_site_scopes, scopes_to_string};
use crate::models::access_token::{AccessToken, AccessTokenKind, AccessTokenResponse};
use crate::repository::traits::AccessTokenRepository;

#[derive(Clone)]
pub struct AccessTokenService {
    access_token_repo: Arc<dyn AccessTokenRepository>,
    hmac_secret: String,
}

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Invalid scope: {0}")]
    InvalidScope(String),

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
            TokenError::InvalidScope(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
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

    pub fn validate_scopes(&self, kind: AccessTokenKind, scopes: Vec<String>) -> Result<Vec<String>, TokenError> {
        let allowed: std::collections::BTreeSet<String> = match kind {
            AccessTokenKind::Instance => default_instance_scopes().into_iter().map(ToString::to_string).collect(),
            AccessTokenKind::Site => default_site_scopes().into_iter().map(ToString::to_string).collect(),
        };

        let scopes = if scopes.is_empty() {
            allowed.iter().cloned().collect()
        } else {
            scopes
        };

        for scope in &scopes {
            if !allowed.contains(scope) {
                return Err(TokenError::InvalidScope(format!("Unsupported scope '{}'", scope)));
            }
        }

        Ok(scopes)
    }

    fn build_token(kind: AccessTokenKind) -> String {
        let random_chars = Uuid::new_v4().to_string().replace('-', "");
        format!("{}{}", kind.prefix(), random_chars)
    }

    async fn create_token_record(
        &self,
        kind: AccessTokenKind,
        site_id: Option<&str>,
        name: String,
        scopes: Vec<String>,
        created_by_user_id: Option<&str>,
    ) -> Result<AccessTokenResponse, TokenError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(TokenError::NameRequired);
        }

        let raw_token = Self::build_token(kind.clone());
        let prefix: String = raw_token.chars().take(24).collect();
        let token_hash = hash(&raw_token, DEFAULT_COST).map_err(|e| TokenError::HashError(e.to_string()))?;
        let token_hmac = compute_key_hmac(&raw_token, &self.hmac_secret);
        let id = Uuid::now_v7().to_string();
        let scope_refs = scopes.iter().map(String::as_str).collect::<Vec<_>>();
        let scopes_string = scopes_to_string(&scope_refs);

        self.access_token_repo
            .create(
                &id,
                kind.clone(),
                site_id,
                name,
                &token_hash,
                &prefix,
                &token_hmac,
                &scopes_string,
                created_by_user_id,
            )
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))?;

        Ok(AccessTokenResponse {
            id,
            kind: kind.to_string(),
            site_id: site_id.map(ToString::to_string),
            name: name.to_string(),
            token: raw_token,
            token_prefix: prefix,
            scopes,
            created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }

    pub async fn list_site_tokens(&self, site_id: &str) -> Result<Vec<AccessToken>, TokenError> {
        self.access_token_repo
            .list(AccessTokenKind::Site, Some(site_id))
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))
    }

    pub async fn create_site_token(
        &self,
        site_id: &str,
        name: String,
        scopes: Vec<String>,
        created_by: Option<&str>,
    ) -> Result<AccessTokenResponse, TokenError> {
        let scopes = self.validate_scopes(AccessTokenKind::Site, scopes)?;
        self.create_token_record(AccessTokenKind::Site, Some(site_id), name, scopes, created_by)
            .await
    }

    pub async fn delete_site_token(&self, token_id: &str, site_id: &str) -> Result<u64, TokenError> {
        self.access_token_repo
            .delete(token_id, AccessTokenKind::Site, Some(site_id))
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))
    }

    pub async fn list_instance_tokens(&self) -> Result<Vec<AccessToken>, TokenError> {
        self.access_token_repo
            .list(AccessTokenKind::Instance, None)
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))
    }

    pub async fn create_instance_token(
        &self,
        name: String,
        scopes: Vec<String>,
    ) -> Result<AccessTokenResponse, TokenError> {
        let scopes = self.validate_scopes(AccessTokenKind::Instance, scopes)?;
        self.create_token_record(AccessTokenKind::Instance, None, name, scopes, None)
            .await
    }

    pub async fn delete_instance_token(&self, token_id: &str) -> Result<u64, TokenError> {
        self.access_token_repo
            .delete(token_id, AccessTokenKind::Instance, None)
            .await
            .map_err(|e| TokenError::DatabaseError(e.to_string()))
    }
}
