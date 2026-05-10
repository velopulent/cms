mod access_tokens;
mod auth;
mod collections;
mod dashboard;
mod docs;
mod entry;
mod files;
mod graphql;
mod mcp;
mod openapi;
mod singleton;
mod sites;
mod webhooks;

use std::sync::Arc;

use axum::{Extension, Router};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use tracing::info;

use crate::config::Config;
use crate::middleware::rate_limit::RateLimiter;
use crate::repository::Repository;
use crate::services::Services;
use crate::storage::StorageRegistry;
use crate::tracing::trace_request;

pub fn create_router(
    repository: Repository,
    config: Config,
    storage_registry: Arc<StorageRegistry>,
    services: Services,
) -> Router {
    let rate_limiter = RateLimiter::new(config.rate_limit_max_requests, config.rate_limit_window_secs);

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    let mcp_enabled = config.mcp_enabled;

    let mut router = Router::new()
        .merge(auth::auth_routes())
        .merge(sites::site_routes())
        .merge(access_tokens::access_token_routes())
        .merge(collections::collection_routes())
        .merge(entry::entry_routes())
        .merge(singleton::singleton_routes())
        .merge(files::file_routes(config.max_upload_size_bytes))
        .merge(webhooks::webhook_routes())
        .merge(graphql::graphql_routes())
        .merge(docs::docs_routes())
        .layer(axum::middleware::from_fn_with_state((), trace_request))
        .merge(dashboard::dashboard_routes())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        .layer(Extension(repository.clone()))
        .layer(Extension(config.clone()))
        .layer(Extension(storage_registry.clone()))
        .layer(Extension(services))
        .layer(Extension(rate_limiter));

    if mcp_enabled {
        let mcp_ct = CancellationToken::new();
        let mcp_router = mcp::mcp_routes(Arc::new(repository.into()), Arc::new(config), storage_registry, mcp_ct);
        router = router.merge(mcp_router);
        info!("MCP HTTP endpoint enabled at /mcp");
    } else {
        drop(repository);
    }

    router
}
