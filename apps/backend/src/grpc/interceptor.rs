use std::collections::BTreeSet;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use hmac::digest::KeyInit;
use sha2::Sha256;
use tracing::{Span, error};

use crate::config::Config;
use crate::grpc::auth::{AuthContext, parse_token};
use crate::models::access_token::AccessTokenKind;
use crate::repository::Repository;

/// HMAC type alias for access token validation.
pub type HmacSha256 = Hmac<Sha256>;

/// Computes the HMAC-SHA256 of a token using the provided secret.
///
/// This function is used to validate access tokens against stored HMAC hashes.
/// It provides a consistent hashing mechanism across the authentication
/// layer.
///
/// # Arguments
/// * `key` - The access token to hash
/// * `hmac_secret` - The secret key used for HMAC computation
///
/// # Returns
/// Hex-encoded HMAC-SHA256 string
///
/// # Panics
/// Panics if the HMAC key initialization fails (should never happen with valid UTF-8)
pub fn compute_key_hmac(key: &str, hmac_secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(key.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Authentication context for gRPC requests.
///
/// This struct is injected into request extensions by the authentication
/// layer and contains the authenticated site's ID, permissions, and token type.
/// Handlers can extract this context to perform authorization checks.
#[derive(Clone, Debug)]
pub struct GrpcAuthContext {
    /// The site ID associated with the authenticated site token.
    /// This is populated only for site-scoped tokens.
    pub site_id: Option<String>,
    /// Structured scopes for the authenticated token.
    pub scopes: BTreeSet<String>,
    /// The kind of token used for authentication.
    pub token_kind: AccessTokenKind,
}

impl GrpcAuthContext {
    /// Checks if the authenticated context has the specified scope.
    ///
    /// # Arguments
    /// * `permission` - The permission to check for
    ///
    /// # Returns
    /// `true` if the permissions string contains the permission, `false` otherwise
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.contains(scope)
    }

    /// Checks if the context has content write permissions.
    ///
    /// # Returns
    /// `true` if the permissions include "write" or "admin"
    pub fn can_write(&self) -> bool {
        self.has_scope(crate::middleware::auth::SCOPE_CONTENT_WRITE)
            || self.has_scope(crate::middleware::auth::SCOPE_SCHEMA_WRITE)
            || self.has_scope(crate::middleware::auth::SCOPE_ASSETS_WRITE)
    }

    pub fn can_read(&self) -> bool {
        self.has_scope(crate::middleware::auth::SCOPE_CONTENT_READ)
            || self.has_scope(crate::middleware::auth::SCOPE_SCHEMA_READ)
            || self.has_scope(crate::middleware::auth::SCOPE_ASSETS_READ)
    }

    pub fn require_scope(&self, scope: &str, resource: &str) -> Result<(), tonic::Status> {
        if self.has_scope(scope) {
            Ok(())
        } else {
            Err(tonic::Status::permission_denied(format!(
                "Missing required scope '{}' for {}",
                scope, resource
            )))
        }
    }

    pub fn require_site_id(&self) -> Result<&str, tonic::Status> {
        self.site_id
            .as_deref()
            .ok_or_else(|| tonic::Status::internal("Missing site context for site service"))
    }

    pub fn require_instance_token(&self) -> Result<(), tonic::Status> {
        if self.token_kind == AccessTokenKind::Instance {
            Ok(())
        } else {
            Err(tonic::Status::unauthenticated(
                "This endpoint requires a cms_ik_* token.",
            ))
        }
    }

    pub fn require_site_token(&self) -> Result<(), tonic::Status> {
        if self.token_kind == AccessTokenKind::Site {
            Ok(())
        } else {
            Err(tonic::Status::unauthenticated(
                "This endpoint requires a cms_sk_* token.",
            ))
        }
    }

    pub fn require_instance_scope(&self, scope: &str) -> Result<(), tonic::Status> {
        self.require_instance_token()?;
        self.require_scope(scope, "instance operations")
    }

    pub fn require_site_scope(&self, scope: &str) -> Result<(), tonic::Status> {
        self.require_site_token()?;
        self.require_scope(scope, "site operations")
    }
}

/// Synchronous interceptor that parses the Bearer token and stores a
/// lightweight `AuthContext` in request extensions.
///
/// Database validation is performed asynchronously inside handlers via
/// `get_auth_context`.
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
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let token = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| tonic::Status::unauthenticated("Missing access token"))?;

        let ctx = parse_token(token, &self.config)
            .map_err(|_| tonic::Status::unauthenticated("Invalid access token"))?;

        request.extensions_mut().insert(ctx);
        Ok(request)
    }
}

/// Validate the parsed `AuthContext` against the database.
///
/// This performs the heavy async work: HMAC/bcrypt verification,
/// expiry and revocation checks, last_used update, and scope parsing.
async fn validate_auth(
    ctx: &AuthContext,
    repository: &Repository,
) -> Result<GrpcAuthContext, tonic::Status> {
    let keys = repository
        .access_token
        .find_by_prefix(&ctx.prefix)
        .await
        .map_err(|e| {
            error!(error = ?e, "Database error during access token lookup");
            tonic::Status::internal("Authentication service unavailable")
        })?;

    for (key_id, kind, site_id, stored_hash, stored_hmac, expires_at, revoked_at, scopes) in keys {
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

        if let Some(exp) = expires_at {
            if let Ok(expiry) = chrono::NaiveDateTime::parse_from_str(&exp, "%Y-%m-%d %H:%M:%S") {
                if expiry < chrono::Utc::now().naive_utc() {
                    return Err(tonic::Status::unauthenticated("Access token has expired"));
                }
            }
        }

        if let Err(e) = repository.access_token.update_last_used(&key_id).await {
            tracing::warn!(error = ?e, key_id = %key_id, "Failed to update last_used");
        }

        let parsed_kind = kind
            .parse::<AccessTokenKind>()
            .map_err(|_| tonic::Status::unauthenticated("Invalid access token"))?;

        if let Some(ref site_id) = site_id {
            Span::current().record("site_id", tracing::field::display(site_id));
        }

        return Ok(GrpcAuthContext {
            site_id,
            scopes: crate::middleware::auth::parse_scopes(&scopes),
            token_kind: parsed_kind,
        });
    }

    Err(tonic::Status::unauthenticated("Invalid access token"))
}

/// Extract (and validate) the authentication context from a tonic request.
///
/// Results are cached in request extensions to avoid duplicate DB round-trips
/// when a handler checks auth multiple times.
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
        assert_eq!(hmac1.len(), 64); // SHA256 hex is 64 chars
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
            site_id: Some("site123".to_string()),
            scopes: crate::middleware::auth::parse_scopes("content:read,content:write"),
            token_kind: AccessTokenKind::Site,
        };

        assert!(ctx.has_scope("content:read"));
        assert!(ctx.has_scope("content:write"));
        assert!(ctx.can_write());
        assert!(ctx.can_read());
    }

    #[test]
    fn test_grpc_auth_context_read_only() {
        let ctx = GrpcAuthContext {
            site_id: Some("site456".to_string()),
            scopes: crate::middleware::auth::parse_scopes("content:read"),
            token_kind: AccessTokenKind::Site,
        };

        assert!(ctx.can_read());
        assert!(!ctx.can_write());
    }
}
