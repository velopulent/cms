use hmac::{Hmac, Mac};
use sha2::Sha256;

/// HMAC type alias for API key validation.
pub type HmacSha256 = Hmac<Sha256>;

/// Computes the HMAC-SHA256 of a token using the provided secret.
///
/// This function is used to validate API keys against stored HMAC hashes.
/// It provides a consistent hashing mechanism across the authentication
/// layer.
///
/// # Arguments
/// * `key` - The API key/token to hash
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
/// middleware and contains the authenticated site's ID and permissions.
/// Handlers can extract this context to perform authorization checks.
#[derive(Clone, Debug)]
pub struct GrpcAuthContext {
    /// The site ID associated with the authenticated API key
    pub site_id: String,
    /// The permissions string for the API key (e.g., "read", "write", "admin")
    pub permissions: String,
}

impl GrpcAuthContext {
    /// Checks if the authenticated context has the specified permission.
    ///
    /// # Arguments
    /// * `permission` - The permission to check for
    ///
    /// # Returns
    /// `true` if the permissions string contains the permission, `false` otherwise
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    /// Checks if the context has write permissions.
    ///
    /// # Returns
    /// `true` if the permissions include "write" or "admin"
    pub fn can_write(&self) -> bool {
        self.has_permission("write") || self.has_permission("admin")
    }

    /// Checks if the context has admin permissions.
    ///
    /// # Returns
    /// `true` if the permissions include "admin"
    pub fn is_admin(&self) -> bool {
        self.has_permission("admin")
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
            site_id: "site123".to_string(),
            permissions: "read,write".to_string(),
        };

        assert!(ctx.has_permission("read"));
        assert!(ctx.has_permission("write"));
        assert!(!ctx.has_permission("admin"));
        assert!(ctx.can_write());
        assert!(!ctx.is_admin());
    }

    #[test]
    fn test_grpc_auth_context_admin() {
        let ctx = GrpcAuthContext {
            site_id: "site456".to_string(),
            permissions: "admin".to_string(),
        };

        assert!(ctx.has_permission("admin"));
        assert!(ctx.can_write());
        assert!(ctx.is_admin());
    }
}
