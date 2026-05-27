use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

/// Stub authorization middleware.
///
/// Currently passes all requests through unchanged.
/// Future centralized policy enforcement will plug in here.
pub async fn authz_middleware(request: Request, next: Next) -> Response {
    next.run(request).await
}
