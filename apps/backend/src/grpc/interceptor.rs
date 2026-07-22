use std::sync::Arc;

use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::{Span, error};

use crate::config::Config;
use crate::grpc::auth::{AuthContext, parse_token};
use crate::models::access_token::{AccessTokenPermission, TokenScope, TokenScopes, decode_scopes};
use crate::repository::Repository;

pub type HmacSha256 = Hmac<Sha256>;

pub fn compute_key_hmac(key: &str, hmac_secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(key.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[derive(Clone, Debug)]
pub struct GrpcAuthContext {
    pub token_id: String,
    pub site_id: String,
    pub permission: AccessTokenPermission,
    pub scopes: TokenScopes,
}

impl GrpcAuthContext {
    pub fn can_write(&self) -> bool {
        self.permission.can_write()
    }

    pub fn require_scope(&self, scope: TokenScope) -> Result<(), tonic::Status> {
        if self.scopes.contains(&scope) {
            Ok(())
        } else {
            Err(tonic::Status::permission_denied(
                "Token scope does not permit this operation",
            ))
        }
    }

    pub fn require_site_id(&self) -> Result<&str, tonic::Status> {
        Ok(&self.site_id)
    }
}

#[derive(Clone)]
pub struct AuthInterceptor {
    config: Arc<Config>,
}

impl AuthInterceptor {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        let token = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| tonic::Status::unauthenticated("Missing access token"))?;

        let ctx =
            parse_token(token, &self.config).map_err(|_| tonic::Status::unauthenticated("Invalid access token"))?;

        request.extensions_mut().insert(ctx);
        Ok(request)
    }
}

async fn validate_auth(ctx: &AuthContext, repository: &Repository) -> Result<GrpcAuthContext, tonic::Status> {
    let keys = repository.access_token.find_by_prefix(&ctx.prefix).await.map_err(|e| {
        error!(error = ?e, "Database error during access token lookup");
        tonic::Status::internal("Authentication service unavailable")
    })?;

    for (key_id, site_id, stored_hash, stored_hmac, expires_at, revoked_at, permission, last_used_at) in keys {
        let valid = if let Some(ref stored) = stored_hmac {
            stored == &ctx.hmac
        } else {
            bcrypt::verify(&ctx.token, &stored_hash).unwrap_or(false)
        };

        if !valid {
            continue;
        }

        if revoked_at.is_some() {
            return Err(tonic::Status::unauthenticated("Invalid access token"));
        }

        if let Some(exp) = expires_at
            && let Ok(expiry) = chrono::NaiveDateTime::parse_from_str(&exp, "%Y-%m-%d %H:%M:%S")
            && expiry < chrono::Utc::now().naive_utc()
        {
            return Err(tonic::Status::unauthenticated("Access token has expired"));
        }

        if crate::middleware::auth::needs_touch(last_used_at.as_deref())
            && let Err(e) = repository.access_token.update_last_used(&key_id).await
        {
            tracing::warn!(error = ?e, key_id = %key_id, "Failed to update last_used");
        }

        Span::current().record("site_id", tracing::field::display(&site_id));

        let scopes = decode_scopes(&permission).map_err(|_| tonic::Status::unauthenticated("Invalid access token"))?;
        let can_write = scopes.iter().any(|scope| {
            matches!(
                scope,
                TokenScope::SiteSettingsWrite
                    | TokenScope::ContentWrite
                    | TokenScope::FilesWrite
                    | TokenScope::SchemaWrite
                    | TokenScope::WebhooksWrite
                    | TokenScope::DeploymentsWrite
            )
        });
        return Ok(GrpcAuthContext {
            token_id: key_id,
            site_id,
            permission: if can_write {
                AccessTokenPermission::Write
            } else {
                AccessTokenPermission::Read
            },
            scopes,
        });
    }

    Err(tonic::Status::unauthenticated("Invalid access token"))
}

pub async fn get_auth_context<T>(
    request: &mut tonic::Request<T>,
    repository: &Repository,
) -> Result<GrpcAuthContext, tonic::Status> {
    if let Some(ctx) = request.extensions().get::<GrpcAuthContext>() {
        return Ok(ctx.clone());
    }

    let auth_ctx = request
        .extensions()
        .get::<AuthContext>()
        .ok_or_else(|| tonic::Status::internal("Missing auth context"))?;

    let validated = validate_auth(auth_ctx, repository).await?;
    request.extensions_mut().insert(validated.clone());
    Ok(validated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_key_hmac_consistency() {
        let secret = "my_secret_key";
        let token = "cms_test_token";

        let hmac1 = compute_key_hmac(token, secret);
        let hmac2 = compute_key_hmac(token, secret);

        assert_eq!(hmac1, hmac2);
        assert_eq!(hmac1.len(), 64);
    }

    #[test]
    fn test_compute_key_hmac_different_inputs() {
        let secret = "my_secret_key";

        let hmac1 = compute_key_hmac("token1", secret);
        let hmac2 = compute_key_hmac("token2", secret);

        assert_ne!(hmac1, hmac2);
    }

    #[test]
    fn test_grpc_auth_context_permissions() {
        let ctx = GrpcAuthContext {
            token_id: "token123".to_string(),
            site_id: "site123".to_string(),
            permission: AccessTokenPermission::Write,
            scopes: [TokenScope::ContentWrite].into_iter().collect(),
        };

        assert!(ctx.can_write());
        assert!(ctx.require_scope(TokenScope::ContentWrite).is_ok());
        assert!(ctx.require_scope(TokenScope::ContentRead).is_err());
    }

    #[test]
    fn test_grpc_auth_context_read_only() {
        let ctx = GrpcAuthContext {
            token_id: "token456".to_string(),
            site_id: "site456".to_string(),
            permission: AccessTokenPermission::Read,
            scopes: [TokenScope::ContentRead].into_iter().collect(),
        };

        assert!(!ctx.can_write());
    }
}
