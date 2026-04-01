mod api_keys;
mod auth;
mod collections;
mod content;
mod docs;
mod files;
mod graphql;
mod openapi;
mod sites;

use axum::{
    Extension, Router,
    routing::get,
};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;

use crate::config::Config;
use crate::handlers::file_handler::StorageManager;
use crate::handlers::ui_handler::ui_handler;

pub fn create_router(pool: SqlitePool, config: Config, storage: StorageManager) -> Router {
    Router::new()
        .merge(auth::auth_routes())
        .merge(sites::site_routes())
        .merge(api_keys::api_key_routes())
        .merge(collections::collection_routes())
        .merge(content::content_routes())
        .merge(files::file_routes(config.max_upload_size_bytes))
        .merge(graphql::graphql_routes())
        .merge(docs::docs_routes())
        // SPA fallback — must be last
        .route(
            "/",
            get(|| async { ui_handler(axum::extract::Path("".into())).await }),
        )
        .route("/{*file}", get(ui_handler))
        .layer(CorsLayer::permissive())
        .layer(Extension(pool))
        .layer(Extension(config))
        .layer(Extension(storage))
}
