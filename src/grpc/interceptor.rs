use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeSet;

use crate::models::access_token::AccessTokenKind;

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
/// middleware and contains the authenticated site's ID, permissions, and token type.
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

/// Legacy synchronous interceptor for simple use cases.
///
/// **DEPRECATED**: This interceptor uses `block_on` which can cause runtime panics
/// when called from within an async context. Use `AuthLayer`/`AuthMiddleware` instead
/// for production code.
///
/// This is kept for backward compatibility with existing code that may use
/// `with_interceptor` directly. New code should use the Tower middleware approach.
#[derive(Clone)]
pub struct AuthInterceptor;

impl AuthInterceptor {
    /// Creates a new auth interceptor.
    ///
    /// **Note**: This creates a placeholder interceptor that performs no
    /// actual authentication. Use `AuthLayer` for real authentication.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuthInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        // This is a no-op interceptor kept for compatibility.
        // Real authentication is handled by AuthMiddleware.
        //
        // If auth context was already injected by the middleware, pass it through.
        // Otherwise, the request will proceed without auth (handlers should check
        // for auth context and reject if required).
        Ok(request)
    }
}

/// Extracts the authentication context from a tonic request.
///
/// This function is used by gRPC service handlers to retrieve the
/// authentication context that was injected by the middleware.
///
/// # Type Parameters
/// * `T` - The request message type
///
/// # Arguments
/// * `request` - The tonic request
///
/// # Returns
/// * `Ok(GrpcAuthContext)` - Auth context found in request extensions
/// * `Err(tonic::Status)` - Auth context not found (middleware not applied or auth failed)
///
/// # Example
/// ```rust,ignore
/// async fn my_handler(
///     &self,
///     request: tonic::Request<MyRequest>,
/// ) -> Result<tonic::Response<MyResponse>, tonic::Status> {
///     let auth = get_auth_context(&request)?;
///     println!("Authenticated site: {}", auth.site_id);
///     // ... handle request
/// }
/// ```
pub fn get_auth_context<T>(request: &tonic::Request<T>) -> Result<GrpcAuthContext, tonic::Status> {
    request
        .extensions()
        .get::<GrpcAuthContext>()
        .cloned()
        .ok_or_else(|| tonic::Status::unauthenticated("Authentication required"))
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
