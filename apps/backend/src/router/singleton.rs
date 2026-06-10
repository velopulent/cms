use axum::{
    Router,
    routing::{get, put},
};

use crate::handlers::singleton_handler::{get_singleton, list_singletons, update_singleton};

pub fn public_routes() -> Router {
    Router::new()
        .route("/singletons", get(list_singletons))
        .route("/singletons/{slug}", get(get_singleton))
        .route("/singletons/{slug}", put(update_singleton))
}

pub fn dashboard_routes() -> Router {
    Router::new()
        .route("/singletons", get(list_singletons))
        .route("/singletons/{slug}", get(get_singleton))
        .route("/singletons/{slug}", put(update_singleton))
}
