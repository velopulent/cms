mod api_keys;
mod auth;
mod collections;
mod content;
mod docs;
mod files;
mod graphql;
mod openapi;
mod singleton;
mod sites;

use axum::{
    Extension, Router,
    routing::get,
};
use tower_http::cors::CorsLayer;

use crate::config::Config;
use crate::handlers::file_handler::StorageManager;
use crate::handlers::ui_handler::ui_handler;
use crate::repository::Repository;

pub fn create_router(repository: Repository, config: Config, storage: StorageManager) -> Router {
    Router::new()
        .merge(auth::auth_routes())
        .merge(sites::site_routes())
        .merge(api_keys::api_key_routes())
        .merge(collections::collection_routes())
        .merge(content::content_routes())
        .merge(singleton::singleton_routes())
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
        .layer(Extension(repository))
        .layer(Extension(config))
        .layer(Extension(storage))
}
