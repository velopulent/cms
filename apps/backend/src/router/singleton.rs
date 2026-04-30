use axum::{
    Router,
    routing::{get, put},
};

use crate::handlers::singleton_handler::{get_singleton, list_singletons, update_singleton};

pub fn singleton_routes() -> Router {
    Router::new()
        .route("/api/v1/singletons", get(list_singletons))
        .route("/api/v1/singletons/{slug}", get(get_singleton))
        .route("/api/v1/singletons/{slug}", put(update_singleton))
}
