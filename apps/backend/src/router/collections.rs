use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::collection_handler::{
    create_collection, delete_collection, get_collection, list_collections, update_collection,
};

pub fn public_routes() -> Router {
    Router::new()
        .route("/collections", get(list_collections))
        .route("/collections", post(create_collection))
        .route("/collections/{collection_slug}", get(get_collection))
        .route("/collections/{collection_slug}", put(update_collection))
        .route("/collections/{collection_slug}", delete(delete_collection))
}

pub fn dashboard_routes() -> Router {
    Router::new()
        .route("/collections", get(list_collections))
        .route("/collections", post(create_collection))
        .route("/collections/{collection_slug}", get(get_collection))
        .route("/collections/{collection_slug}", put(update_collection))
        .route("/collections/{collection_slug}", delete(delete_collection))
}
