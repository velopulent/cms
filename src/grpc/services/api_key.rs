use std::sync::Arc;

use bcrypt::{DEFAULT_COST, hash};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::grpc::cms::v1::api_key_service_server::ApiKeyService;
use crate::grpc::cms::v1::{
    ApiKey as ProtoApiKey, CreateApiKeyRequest, CreateApiKeyResponse as ProtoCreateApiKeyResponse, DeleteApiKeyRequest,
    DeleteResponse, ListApiKeysRequest, ListApiKeysResponse,
};
use crate::grpc::interceptor::get_auth_context;
use crate::middleware::auth::compute_key_hmac;
use crate::models::api_key::ApiKey;
use crate::repository::Repository;

#[derive(Clone)]
pub struct ApiKeyServiceImpl {
    repository: Arc<Repository>,
    hmac_secret: String,
}

impl ApiKeyServiceImpl {
    pub fn new(repository: Arc<Repository>, hmac_secret: String) -> Self {
        Self {
            repository,
            hmac_secret,
        }
    }
}

#[tonic::async_trait]
impl ApiKeyService for ApiKeyServiceImpl {
    async fn list_api_keys(
        &self,
        request: Request<ListApiKeysRequest>,
    ) -> Result<Response<ListApiKeysResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

        let keys = self
            .repository
            .api_key
            .list(&site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let response = ListApiKeysResponse {
            api_keys: keys.into_iter().map(ProtoApiKey::from).collect(),
        };

        Ok(Response::new(response))
    }

    async fn create_api_key(
        &self,
        request: Request<CreateApiKeyRequest>,
    ) -> Result<Response<ProtoCreateApiKeyResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;

        let req = request.into_inner();

        if req.name.trim().is_empty() {
            return Err(Status::invalid_argument("Name is required"));
        }

        let permissions = match req.permissions.as_str() {
            "read" | "" => "read",
            "write" => "write",
            _ => {
                return Err(Status::invalid_argument(
                    "Invalid permissions. Must be 'read' or 'write'",
                ));
            }
        };

        let random_chars = Uuid::new_v4().to_string().replace('-', "");
        let segment_a: String = random_chars.chars().take(8).collect();
        let segment_b: String = random_chars.chars().skip(8).take(24).collect();
        let raw_key = format!("cms_{}_{}", segment_a, segment_b);

        let prefix: String = raw_key.chars().take(16).collect();

        let key_hash = hash(&raw_key, DEFAULT_COST).map_err(|e| Status::internal(format!("Hash error: {}", e)))?;

        let key_hmac = compute_key_hmac(&raw_key, &self.hmac_secret);

        let id = Uuid::now_v7().to_string();

        self.repository
            .api_key
            .create(&id, &site_id, &req.name, &key_hash, &prefix, &key_hmac, permissions)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let api_key = ProtoApiKey {
            id: id.clone(),
            site_id,
            name: req.name,
            key_prefix: prefix,
            permissions: permissions.to_string(),
            last_used_at: None,
            created_at: now,
            expires_at: req.expires_at,
        };

        Ok(Response::new(ProtoCreateApiKeyResponse {
            api_key: Some(api_key),
            key: raw_key,
        }))
    }

    async fn delete_api_key(&self, request: Request<DeleteApiKeyRequest>) -> Result<Response<DeleteResponse>, Status> {
        let auth = get_auth_context(&request)?;
        let site_id = auth.site_id;
        let id = request.into_inner().id;

        let deleted = self
            .repository
            .api_key
            .delete(&id, &site_id)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(DeleteResponse {
            success: deleted > 0,
            message: if deleted > 0 {
                "API key deleted".to_string()
            } else {
                "API key not found".to_string()
            },
        }))
    }
}

impl From<ApiKey> for ProtoApiKey {
    fn from(k: ApiKey) -> Self {
        ProtoApiKey {
            id: k.id,
            site_id: k.site_id,
            name: k.name,
            key_prefix: k.key_prefix,
            permissions: k.permissions,
            last_used_at: k.last_used_at,
            created_at: k.created_at,
            expires_at: k.expires_at,
        }
    }
}
