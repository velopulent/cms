use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http::{HeaderMap, Request, Response, StatusCode};
use tonic::body::BoxBody;
use tower::{Layer, Service};
use tracing::{Span, error};

use crate::config::Config;
use crate::grpc::interceptor::{GrpcAuthContext, compute_key_hmac};
use crate::models::access_token::AccessTokenKind;
use crate::repository::Repository;

/// Tower Layer for gRPC authentication middleware.
///
/// This layer wraps gRPC services to provide access token authentication
/// using async database lookups. It properly handles authentication
/// within the Tokio async runtime without blocking.
#[derive(Clone)]
pub struct AuthLayer {
    repository: Arc<Repository>,
    config: Arc<Config>,
}

impl AuthLayer {
    pub fn new(repository: Arc<Repository>, config: Arc<Config>) -> Self {
        Self { repository, config }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware {
            inner,
            repository: self.repository.clone(),
            config: self.config.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    repository: Arc<Repository>,
    config: Arc<Config>,
}

impl<S> Service<Request<BoxBody>> for AuthMiddleware<S>
where
    S: Service<Request<BoxBody>, Response = Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = S::Response;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request<BoxBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        let repository = self.repository.clone();
        let config = self.config.clone();

        let auth_header = extract_auth_header(request.headers());

        Box::pin(async move {
            match authenticate_request(auth_header, &repository, &config).await {
                Ok(auth_context) => {
                    let mut request = request;
                    request.extensions_mut().insert(auth_context);
                    inner.call(request).await.map_err(Into::into)
                }
                Err(auth_error) => {
                    error!(error = ?auth_error, "gRPC authentication failed");
                    let status = match auth_error {
                        AuthError::MissingToken => tonic::Status::unauthenticated("Missing access token"),
                        AuthError::InvalidFormat => tonic::Status::unauthenticated("Invalid access token format"),
                        AuthError::Expired => tonic::Status::unauthenticated("Access token has expired"),
                        AuthError::InvalidKey => tonic::Status::unauthenticated("Invalid access token"),
                        AuthError::Database => tonic::Status::internal("Authentication service unavailable"),
                    };
                    let response = status_to_response(status);
                    Ok(response)
                }
            }
        })
    }
}

#[derive(Debug)]
enum AuthError {
    MissingToken,
    InvalidFormat,
    Expired,
    InvalidKey,
    Database,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingToken => write!(f, "Missing access token"),
            AuthError::InvalidFormat => write!(f, "Invalid access token format"),
            AuthError::Expired => write!(f, "Access token has expired"),
            AuthError::InvalidKey => write!(f, "Invalid access token"),
            AuthError::Database => write!(f, "Database error during authentication"),
        }
    }
}

impl std::error::Error for AuthError {}

fn extract_auth_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

async fn authenticate_request(
    auth_header: Option<String>,
    repository: &Repository,
    config: &Config,
) -> Result<GrpcAuthContext, AuthError> {
    let token = auth_header
        .as_deref()
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingToken)?;

    if !token.starts_with("cms_") {
        return Err(AuthError::InvalidFormat);
    }

    let prefix: String = token.chars().take(24).collect();
    let token_hmac = compute_key_hmac(token, &config.hmac_secret);

    let keys = repository.access_token.find_by_prefix(&prefix).await.map_err(|e| {
        error!(error = ?e, "Database error during access token lookup");
        AuthError::Database
    })?;

    for (key_id, kind, site_id, stored_hash, stored_hmac, expires_at, revoked_at, scopes) in keys {
        let valid = if let Some(ref stored) = stored_hmac {
            stored == &token_hmac
        } else {
            bcrypt::verify(token, &stored_hash).unwrap_or(false)
        };

        if !valid {
            continue;
        }

        if revoked_at.is_some() {
            return Err(AuthError::InvalidKey);
        }

        if let Some(exp) = expires_at {
            if let Ok(expiry) = chrono::NaiveDateTime::parse_from_str(&exp, "%Y-%m-%d %H:%M:%S") {
                if expiry < chrono::Utc::now().naive_utc() {
                    return Err(AuthError::Expired);
                }
            }
        }

        if let Err(e) = repository.access_token.update_last_used(&key_id).await {
            tracing::warn!(error = ?e, key_id = %key_id, "Failed to update access token last_used timestamp");
        }

        let parsed_kind = kind.parse::<AccessTokenKind>().map_err(|_| AuthError::InvalidKey)?;

        if let Some(ref site_id) = site_id {
            Span::current().record("site_id", tracing::field::display(site_id));
        }

        return Ok(GrpcAuthContext {
            site_id,
            scopes: crate::middleware::auth::parse_scopes(&scopes),
            token_kind: parsed_kind,
        });
    }

    Err(AuthError::InvalidKey)
}

/// Converts a tonic::Status into an HTTP Response.
///
/// This is used to return gRPC errors from the middleware layer
/// before the request reaches the tonic service.
fn status_to_response(status: tonic::Status) -> Response<BoxBody> {
    // Create a gRPC-style HTTP response
    // gRPC status is encoded in the grpc-status header
    let (mut parts, _body) = Response::new(()).into_parts();

    // Set the grpc-status header (convert code to string first)
    let code: i32 = status.code().into();
    if let Ok(code_value) = http::HeaderValue::from_str(&code.to_string()) {
        parts.headers.insert("grpc-status", code_value);
    }

    // Set the grpc-message header with the error message
    if let Ok(message) = http::HeaderValue::from_str(status.message()) {
        parts.headers.insert("grpc-message", message);
    }

    // Set content-type for gRPC
    parts
        .headers
        .insert("content-type", http::HeaderValue::from_static("application/grpc"));

    // For unauthorized responses, also set HTTP status
    if status.code() == tonic::Code::Unauthenticated {
        parts.status = StatusCode::UNAUTHORIZED;
    } else {
        parts.status = StatusCode::OK; // gRPC always returns 200 OK, error in headers
    }

    Response::from_parts(parts, BoxBody::default())
}

/// Convenience function to get auth context from a tonic request.
///
/// This function is used by gRPC handlers to extract the authentication
/// context that was injected by the middleware.
///
/// # Type Parameters
/// * `T` - The request message type
///
/// # Arguments
/// * `request` - The tonic request
///
/// # Returns
/// * `Ok(GrpcAuthContext)` - Auth context found
/// * `Err(tonic::Status)` - Auth context not found (should not happen)
pub fn get_auth_context<T>(request: &tonic::Request<T>) -> Result<GrpcAuthContext, tonic::Status> {
    request
        .extensions()
        .get::<GrpcAuthContext>()
        .cloned()
        .ok_or_else(|| tonic::Status::internal("Auth context not found in request"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_auth_header_present() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            http::HeaderValue::from_static("Bearer cms_test_token_123"),
        );

        let header = extract_auth_header(&headers);
        assert_eq!(header, Some("Bearer cms_test_token_123".to_string()));
    }

    #[test]
    fn test_extract_auth_header_missing() {
        let headers = HeaderMap::new();
        let header = extract_auth_header(&headers);
        assert_eq!(header, None);
    }

    #[test]
    fn test_extract_auth_header_invalid_utf8() {
        use http::header::HeaderValue;

        let mut headers = HeaderMap::new();
        // Insert invalid UTF-8 bytes
        headers.insert("authorization", HeaderValue::from_bytes(&[0x80, 0x81, 0x82]).unwrap());

        let header = extract_auth_header(&headers);
        assert_eq!(header, None);
    }

    #[test]
    fn test_status_to_response_unauthenticated() {
        let status = tonic::Status::unauthenticated("Invalid access token");
        let response = status_to_response(status);

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("grpc-status"),
            Some(&http::HeaderValue::from_static("16")) // Code::Unauthenticated = 16
        );
        assert!(
            response
                .headers()
                .get("grpc-message")
                .unwrap()
                .as_bytes()
                .starts_with(b"Invalid access token")
        );
    }

    #[test]
    fn test_status_to_response_internal() {
        let status = tonic::Status::internal("Something went wrong");
        let response = status_to_response(status);

        assert_eq!(response.status(), StatusCode::OK); // gRPC returns 200 even for errors
        assert_eq!(
            response.headers().get("grpc-status"),
            Some(&http::HeaderValue::from_static("13")) // Code::Internal = 13
        );
    }
}
