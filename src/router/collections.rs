use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::collection_handler::{
    create_collection, delete_collection, get_collection, list_collections, update_collection,
};

pub fn collection_routes() -> Router {
    Router::new()
        .route("/api/v1/sites/{site_id}/collections", get(list_collections))
        .route("/api/v1/sites/{site_id}/collections", post(create_collection))
        .route(
            "/api/v1/sites/{site_id}/collections/{collection_slug}",
            get(get_collection),
        )
        .route(
            "/api/v1/sites/{site_id}/collections/{collection_slug}",
            put(update_collection),
        )
        .route(
            "/api/v1/sites/{site_id}/collections/{collection_slug}",
            delete(delete_collection),
        )
}
