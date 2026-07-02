mod access_tokens;
mod auth;
mod backup;
mod collections;
mod dashboard;
mod docs;
mod entry;
mod files;
mod graphql;
mod instance;
mod mcp;
mod openapi;
mod singleton;
mod sites;
mod webhooks;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, Method, header};
use axum::{Extension, Router, middleware::from_fn, routing::get};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::handlers::site_handler::get_current_site;
use crate::middleware::api_auth::api_auth_middleware;
use crate::middleware::authz::authz_middleware;
use crate::middleware::dashboard_auth::dashboard_auth_middleware;
use crate::middleware::rate_limit::RateLimiter;
use crate::middleware::site_resolver::{api_site_resolver, dashboard_site_resolver};
use crate::repository::Repository;
use crate::services::Services;
use crate::services::backup::BackupService;
use crate::storage::StorageRegistry;
use crate::tracing::trace_request;

/// Public API v1 resource routes wrapped in Auth → SiteResolver → AuthZ middleware.
fn public_api_v1_routes(max_upload_bytes: usize) -> Router {
    let resource_routes = Router::new()
        .merge(collections::public_routes())
        .merge(entry::public_routes())
        .merge(singleton::public_routes())
        .merge(webhooks::public_routes())
        .merge(files::public_routes(max_upload_bytes));

    Router::new()
        .merge(resource_routes)
        // Inner — runs third (after site resolver sets RequestContext)
        .layer(from_fn(authz_middleware))
        // Middle — runs second (after api_auth sets Actor)
        .layer(from_fn(api_site_resolver))
        // Outer — runs first (validates Bearer vcms_site_* token)
        .layer(from_fn(api_auth_middleware))
}

/// Site-scoped dashboard routes: Auth → SiteResolver → AuthZ.
fn dashboard_site_v1_routes(max_upload_bytes: usize) -> Router {
    let site_routes = Router::new()
        .merge(sites::dashboard_site_routes())
        .merge(collections::dashboard_routes())
        .merge(entry::dashboard_routes())
        .merge(singleton::dashboard_routes())
        .merge(webhooks::dashboard_routes())
        .merge(access_tokens::dashboard_routes())
        .merge(backup::site_routes())
        .merge(files::dashboard_routes(max_upload_bytes));

    Router::new()
        .merge(site_routes)
        // Inner — runs third
        .layer(from_fn(authz_middleware))
        // Middle — runs second (reads {site_id} from path, builds RequestContext)
        .layer(from_fn(dashboard_site_resolver))
}

pub fn create_router(
    repository: Repository,
    config: Config,
    storage_registry: Arc<StorageRegistry>,
    services: Services,
    backup: Arc<BackupService>,
) -> Router {
    let rate_limiter = RateLimiter::new(
        config.rate_limit_max_requests,
        config.rate_limit_window_secs,
        config.trust_proxy_headers,
    );
    let max_upload_bytes = config.max_upload_size_bytes;

    let cors = if config.allowed_origins.is_empty() {
        // No cross-origin access configured: same-origin only. Emitting no
        // Access-Control-Allow-Origin header makes browsers block cross-site reads.
        CorsLayer::new()
    } else {
        let origins = config
            .allowed_origins
            .iter()
            .filter_map(|origin| origin.parse().ok())
            .collect::<Vec<_>>();
        // Explicit method + header allow-lists are required for credentialed CORS;
        // `Any` is silently ignored by browsers when credentials are allowed.
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                header::ACCEPT,
                HeaderName::from_static("x-csrf-token"),
            ])
            .allow_credentials(true)
    };

    let mcp_enabled = config.mcp_enabled;
    // Share the single `Services` (and its single-writer search index) with the
    // MCP HTTP server instead of constructing a second one.
    let mcp_services = if mcp_enabled {
        Some(Arc::new(services.clone()))
    } else {
        None
    };

    let mut router = Router::new()
        // ── Auth (no middleware) ──
        .merge(auth::auth_routes())
        // ── Public API (/api/v1/*) ──
        .merge(public_api_v1_routes(max_upload_bytes))
        .route(
            "/api/v1/site",
            get(get_current_site)
                .layer(from_fn(api_site_resolver))
                .layer(from_fn(api_auth_middleware)),
        )
        // ── File serving (no auth — file IDs are effectively opaque) ──
        .merge(files::file_serve_routes())
        // ── Signed-URL upload (no auth — the HMAC token is the credential) ──
        .merge(files::signed_upload_routes(max_upload_bytes))
        // ── Dashboard API (/api/dashboard/*) ──
        .nest(
            "/api/dashboard",
            Router::new()
                .nest("/sites/{site_id}", dashboard_site_v1_routes(max_upload_bytes))
                .merge(sites::dashboard_list_routes())
                .merge(instance::routes())
                .merge(backup::instance_routes())
                .layer(from_fn(dashboard_auth_middleware)),
        )
        // ── GraphQL (custom auth in handler) ──
        .merge(graphql::graphql_routes(config.production))
        // ── Docs ──
        .merge(docs::docs_routes())
        // ── Dashboard SPA ──
        .merge(dashboard::dashboard_routes())
        // ── Global layers ──
        .layer(from_fn(trace_request))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(Extension(repository.clone()))
        .layer(Extension(config.clone()))
        .layer(Extension(storage_registry.clone()))
        .layer(Extension(services))
        .layer(Extension(backup))
        .layer(Extension(rate_limiter));

    if mcp_enabled {
        let mcp_ct = CancellationToken::new();
        let mcp_router = mcp::mcp_routes(
            mcp_services.expect("mcp services present when mcp enabled"),
            Arc::new(repository),
            Arc::new(config),
            storage_registry,
            mcp_ct,
        );
        router = router.merge(mcp_router);
    } else {
        drop(repository);
    }

    router
}
