pub mod context;
pub mod loaders;
pub mod mutation;
pub mod query;
pub mod schema;
pub mod types;

/// Map an internal/database failure to a generic client-facing GraphQL error,
/// logging the real cause server-side. Use for failures that should never leak
/// implementation detail (DB errors, storage errors); user-facing validation
/// and not-found messages should be surfaced directly instead.
pub fn internal_error(context: &str, e: impl std::fmt::Display) -> async_graphql::Error {
    tracing::error!("graphql {context} error: {e}");
    async_graphql::Error::new("Internal server error")
}

/// Translate domain failures while retaining safe, actionable public messages.
pub fn service_error(context: &str, error: impl Into<crate::services::error::ServiceError>) -> async_graphql::Error {
    use async_graphql::ErrorExtensions;
    let error = error.into();
    let status = error.status_code();
    if status.is_server_error() {
        tracing::error!(context, error = ?error, "GraphQL operation failed");
    }
    async_graphql::Error::new(error.error_message()).extend_with(|_, extensions| {
        extensions.set(
            "code",
            match status.as_u16() {
                400 | 422 => "INVALID_INPUT",
                401 => "UNAUTHENTICATED",
                403 => "FORBIDDEN",
                404 => "NOT_FOUND",
                409 => "CONFLICT",
                412 => "PRECONDITION_FAILED",
                413 => "PAYLOAD_TOO_LARGE",
                _ => "INTERNAL_ERROR",
            },
        );
    })
}
