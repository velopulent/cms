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
