use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::grpc::cms::v1::token_service_server::TokenService;
use crate::grpc::cms::v1::{
    AccessToken as ProtoAccessToken, CreateInstanceTokenRequest, CreateSiteTokenRequest, CreateTokenResponse,
    DeleteInstanceTokenRequest, DeleteResponse, DeleteSiteTokenRequest, ListInstanceTokensRequest,
    ListInstanceTokensResponse, ListSiteTokensRequest, ListSiteTokensResponse,
};
use crate::grpc::interceptor::get_auth_context;
use crate::middleware::auth::{SCOPE_TOKENS_READ, SCOPE_TOKENS_WRITE};
use crate::models::access_token::AccessToken;
use crate::services::access_token::AccessTokenService;

#[derive(Clone)]
pub struct AdminTokenServiceImpl {
    app_token_service: Arc<AccessTokenService>,
}

impl AdminTokenServiceImpl {
    pub fn new(token_service: Arc<AccessTokenService>) -> Self {
        Self {
            app_token_service: token_service,
        }
    }
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
            .app_token_service
            .list_instance_tokens()
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

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

        let response = self
            .app_token_service
            .create_instance_token(req.name, req.scopes)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(CreateTokenResponse {
            access_token: Some(ProtoAccessToken {
                id: response.id,
                kind: response.kind,
                site_id: response.site_id,
                name: response.name,
                token_prefix: response.token_prefix,
                scopes: response.scopes,
                created_by_user_id: None,
                last_used_at: None,
                created_at: response.created_at,
                expires_at: None,
                revoked_at: None,
            }),
            token: response.token,
        }))
    }

    async fn delete_instance_token(
        &self,
        request: Request<DeleteInstanceTokenRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_WRITE, "instance tokens")?;
        let token_id = request.into_inner().token_id;

        let deleted = self
            .app_token_service
            .delete_instance_token(&token_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

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
            .app_token_service
            .list_site_tokens(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

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

        let response = self
            .app_token_service
            .create_site_token(&req.site_id, req.name, req.scopes, None)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

        Ok(Response::new(CreateTokenResponse {
            access_token: Some(ProtoAccessToken {
                id: response.id,
                kind: response.kind,
                site_id: response.site_id,
                name: response.name,
                token_prefix: response.token_prefix,
                scopes: response.scopes,
                created_by_user_id: None,
                last_used_at: None,
                created_at: response.created_at,
                expires_at: None,
                revoked_at: None,
            }),
            token: response.token,
        }))
    }

    async fn delete_site_token(
        &self,
        request: Request<DeleteSiteTokenRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        auth.require_scope(SCOPE_TOKENS_WRITE, "site tokens")?;
        let req = request.into_inner();

        let deleted = self
            .app_token_service
            .delete_site_token(&req.token_id, &req.site_id)
            .await
            .map_err(|e| Status::internal(format!("Error: {}", e)))?;

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
