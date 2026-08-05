mod access_tokens;
mod auth;
mod backup;
mod collections;
mod dashboard;
mod deployments;
mod docs;
mod entry;
mod files;
mod graphql;
mod health;
mod instance;
mod mcp;
mod openapi;
mod singleton;
mod sites;
mod webhooks;

use std::sync::Arc;

use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{
    Extension, Router,
    middleware::{Next, from_fn, from_fn_with_state},
    routing::get,
};
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::database::pool::DbPool;
use crate::handlers::site_handler::get_current_site;
use crate::middleware::api_auth::api_auth_middleware;
use crate::middleware::authz::authz_middleware;
use crate::middleware::dashboard_auth::dashboard_auth_middleware;
use crate::middleware::rate_limit::RateLimiter;
use crate::middleware::site_resolver::{api_site_resolver, dashboard_site_resolver};
use crate::repository::Repository;
use crate::services::Services;
use crate::services::backup::BackupService;
use crate::services::settings::SettingsService;
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
        .merge(deployments::routes())
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
    pool: DbPool,
    repository: Repository,
    config: Config,
    storage_registry: Arc<StorageRegistry>,
    services: Services,
    backup: Arc<BackupService>,
    settings: SettingsService,
) -> Router {
    let rate_limiter = RateLimiter::new(
        config.rate_limit_max_requests,
        config.rate_limit_window_secs,
        config.trust_proxy_headers,
    );
    let max_upload_bytes = config.max_upload_size_bytes;

    // Share the single `Services` (and its single-writer search index) with the
    // MCP HTTP server instead of constructing a second one.
    let mcp_services = Arc::new(services.clone());

    let mut router = Router::new()
        // Public, minimal operational probes. Detailed failures remain in logs/doctor.
        .merge(health::routes(pool.clone()))
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
                .merge(access_tokens::account_routes())
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
        .layer(from_fn_with_state(settings.clone(), dynamic_runtime_policy))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
        .layer(Extension(repository.clone()))
        .layer(Extension(config.clone()))
        .layer(Extension(storage_registry.clone()))
        .layer(Extension(services))
        .layer(Extension(backup))
        .layer(Extension(settings.clone()))
        .layer(Extension(pool))
        .layer(Extension(rate_limiter));

    let mcp_ct = CancellationToken::new();
    let mcp_router = mcp::mcp_routes(
        mcp_services,
        Arc::new(repository),
        Arc::new(config),
        storage_registry,
        mcp_ct,
    );
    router = router.merge(mcp_router.layer(from_fn_with_state(settings, dynamic_runtime_policy)));

    router
}

async fn dynamic_runtime_policy(State(service): State<SettingsService>, request: Request, next: Next) -> Response {
    let settings = service.current();
    let path = request.uri().path();
    if path.starts_with("/mcp") && !settings.general.mcp_enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.contains("upload")
        && request
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .is_some_and(|size| size > settings.general.upload_limit_mb * 1024 * 1024)
    {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            "upload exceeds the configured instance limit",
        )
            .into_response();
    }

    let origin = request.headers().get(header::ORIGIN).cloned();
    let allowed = origin.as_ref().is_some_and(|origin| {
        origin
            .to_str()
            .is_ok_and(|value| settings.security.allowed_origins.iter().any(|item| item == value))
    });
    if request.method() == Method::OPTIONS && origin.is_some() {
        if !allowed {
            return StatusCode::FORBIDDEN.into_response();
        }
        return cors_response(StatusCode::NO_CONTENT.into_response(), origin);
    }
    let response = next.run(request).await;
    if allowed {
        cors_response(response, origin)
    } else {
        response
    }
}

fn cors_response(mut response: Response<Body>, origin: Option<HeaderValue>) -> Response<Body> {
    if let Some(origin) = origin {
        response
            .headers_mut()
            .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("authorization, content-type, accept, x-csrf-token"),
        );
        response
            .headers_mut()
            .append(header::VARY, HeaderValue::from_static("Origin"));
    }
    response
}
