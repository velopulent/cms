use std::sync::Arc;

use bcrypt::{DEFAULT_COST, hash};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::config::Config;
use crate::grpc::cms::v1::token_service_server::TokenService;
use crate::grpc::cms::v1::{
    AccessToken as ProtoAccessToken, CreateInstanceTokenRequest, CreateSiteTokenRequest, CreateTokenResponse,
    DeleteInstanceTokenRequest, DeleteResponse, DeleteSiteTokenRequest, ListInstanceTokensRequest,
    ListInstanceTokensResponse, ListSiteTokensRequest, ListSiteTokensResponse,
};
use crate::grpc::interceptor::get_auth_context;
use crate::middleware::auth::{
    SCOPE_TOKENS_READ, SCOPE_TOKENS_WRITE, default_instance_scopes, default_site_scopes, scopes_to_string,
};
use crate::models::access_token::{AccessToken, AccessTokenKind};
use crate::repository::Repository;

#[derive(Clone)]
pub struct AdminTokenServiceImpl {
    repository: Arc<Repository>,
    hmac_secret: String,
}

impl AdminTokenServiceImpl {
    pub fn new(repository: Arc<Repository>, config: Arc<Config>) -> Self {
        Self {
            repository,
            hmac_secret: config.hmac_secret.clone(),
        }
    }
}

fn validate_scopes(kind: AccessTokenKind, requested: Vec<String>) -> Result<Vec<String>, Status> {
    let allowed = match kind {
        AccessTokenKind::Instance => default_instance_scopes()
            .into_iter()
            .map(ToString::to_string)
            .collect::<std::collections::BTreeSet<_>>(),
        AccessTokenKind::Site => default_site_scopes()
            .into_iter()
            .map(ToString::to_string)
            .collect::<std::collections::BTreeSet<_>>(),
    };

    let scopes = if requested.is_empty() {
        allowed.iter().cloned().collect()
    } else {
        requested
    };

    for scope in &scopes {
        if !allowed.contains(scope) {
            return Err(Status::invalid_argument(format!("Unsupported scope '{}'", scope)));
        }
    }

    Ok(scopes)
}

async fn create_token_record(
    repository: &Repository,
    hmac_secret: &str,
    kind: AccessTokenKind,
    site_id: Option<&str>,
    name: String,
    scopes: Vec<String>,
) -> Result<CreateTokenResponse, Status> {
    if name.trim().is_empty() {
        return Err(Status::invalid_argument("Name is required"));
    }

    let raw_token = format!("{}{}", kind.prefix(), Uuid::new_v4().to_string().replace('-', ""));
    let prefix: String = raw_token.chars().take(24).collect();
    let token_hash = hash(&raw_token, DEFAULT_COST).map_err(|e| Status::internal(format!("Hash error: {}", e)))?;
    let token_hmac = crate::middleware::auth::compute_key_hmac(&raw_token, hmac_secret);
    let scope_refs = scopes.iter().map(String::as_str).collect::<Vec<_>>();
    let scopes_string = scopes_to_string(&scope_refs);
    let id = Uuid::now_v7().to_string();

    repository
        .access_token
        .create(
            &id,
            kind.clone(),
            site_id,
            &name,
            &token_hash,
            &prefix,
            &token_hmac,
            &scopes_string,
            None,
        )
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

    Ok(CreateTokenResponse {
        access_token: Some(ProtoAccessToken {
            id,
            kind: kind.to_string(),
            site_id: site_id.map(ToString::to_string),
            name,
            token_prefix: prefix,
            scopes,
            created_by_user_id: None,
            last_used_at: None,
            created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            expires_at: None,
            revoked_at: None,
        }),
        token: raw_token,
    })
}

#[tonic::async_trait]
impl TokenService for AdminTokenServiceImpl {
    async fn list_instance_tokens(
        &self,
        request: Request<ListInstanceTokensRequest>,
    ) -> Result<Response<ListInstanceTokensResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_READ, "instance tokens")?;

        let tokens = self
            .repository
            .access_token
            .list(AccessTokenKind::Instance, None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ListInstanceTokensResponse {
            tokens: tokens.into_iter().map(ProtoAccessToken::from).collect(),
        }))
    }

    async fn create_instance_token(
        &self,
        request: Request<CreateInstanceTokenRequest>,
    ) -> Result<Response<CreateTokenResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_WRITE, "instance tokens")?;
        let req = request.into_inner();
        let scopes = validate_scopes(AccessTokenKind::Instance, req.scopes)?;

        Ok(Response::new(
            create_token_record(
                &self.repository,
                &self.hmac_secret,
                AccessTokenKind::Instance,
                None,
                req.name,
                scopes,
            )
            .await?,
        ))
    }

    async fn delete_instance_token(
        &self,
        request: Request<DeleteInstanceTokenRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_WRITE, "instance tokens")?;
        let token_id = request.into_inner().token_id;

        let deleted = self
            .repository
            .access_token
            .delete(&token_id, AccessTokenKind::Instance, None)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Instance token deleted".to_string()
            } else {
                "Instance token not found".to_string()
            },
        }))
    }

    async fn list_site_tokens(
        &self,
        request: Request<ListSiteTokensRequest>,
    ) -> Result<Response<ListSiteTokensResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_READ, "site tokens")?;
        let site_id = request.into_inner().site_id;

        let tokens = self
            .repository
            .access_token
            .list(AccessTokenKind::Site, Some(&site_id))
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ListSiteTokensResponse {
            tokens: tokens.into_iter().map(ProtoAccessToken::from).collect(),
        }))
    }

    async fn create_site_token(
        &self,
        request: Request<CreateSiteTokenRequest>,
    ) -> Result<Response<CreateTokenResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_WRITE, "site tokens")?;
        let req = request.into_inner();
        let scopes = validate_scopes(AccessTokenKind::Site, req.scopes)?;

        Ok(Response::new(
            create_token_record(
                &self.repository,
                &self.hmac_secret,
                AccessTokenKind::Site,
                Some(&req.site_id),
                req.name,
                scopes,
            )
            .await?,
        ))
    }

    async fn delete_site_token(
        &self,
        request: Request<DeleteSiteTokenRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_WRITE, "site tokens")?;
        let req = request.into_inner();

        let deleted = self
            .repository
            .access_token
            .delete(&req.token_id, AccessTokenKind::Site, Some(&req.site_id))
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "Site token deleted".to_string()
            } else {
                "Site token not found".to_string()
            },
        }))
    }
}

impl From<AccessToken> for ProtoAccessToken {
    fn from(token: AccessToken) -> Self {
        Self {
            id: token.id,
            kind: token.kind,
            site_id: token.site_id,
            name: token.name,
            token_prefix: token.token_prefix,
            scopes: token
                .scopes
                .split(',')
                .map(str::trim)
                .filter(|scope| !scope.is_empty())
                .map(ToString::to_string)
                .collect(),
            created_by_user_id: token.created_by_user_id,
            last_used_at: token.last_used_at,
            created_at: token.created_at,
            expires_at: token.expires_at,
            revoked_at: token.revoked_at,
        }
    }
}
