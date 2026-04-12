mod access_tokens;
mod auth;
mod collections;
mod dashboard;
mod docs;
mod entry;
mod files;
mod graphql;
mod openapi;
mod singleton;
mod sites;

use axum::{Extension, Router};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::handlers::file_handler::StorageManager;
use crate::middleware::rate_limit::RateLimiter;
use crate::repository::Repository;
use crate::tracing::trace_request;

pub fn create_router(repository: Repository, config: Config, storage: StorageManager) -> Router {
    let rate_limiter = RateLimiter::new(config.rate_limit_max_requests, config.rate_limit_window_secs);

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    Router::new()
        .merge(auth::auth_routes())
        .merge(sites::site_routes())
        .merge(access_tokens::access_token_routes())
        .merge(collections::collection_routes())
        .merge(entry::entry_routes())
        .merge(singleton::singleton_routes())
        .merge(files::file_routes(config.max_upload_size_bytes))
        .merge(graphql::graphql_routes())
        .merge(docs::docs_routes())
        .layer(axum::middleware::from_fn_with_state((), trace_request))
        // Dashboard
        .merge(dashboard::dashboard_routes())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10MB default body limit
        .layer(Extension(repository))
        .layer(Extension(config))
        .layer(Extension(storage))
        .layer(Extension(rate_limiter))
}
