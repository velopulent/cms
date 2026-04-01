use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::handlers::content_handler::{
    create_content, delete_content, get_content, list_content, publish_content, unpublish_content,
    update_content,
};

pub fn content_routes() -> Router {
    Router::new()
        .route("/api/v1/sites/{site_id}/content", get(list_content))
        .route("/api/v1/sites/{site_id}/content", post(create_content))
        .route("/api/v1/sites/{site_id}/content/{id}", get(get_content))
        .route("/api/v1/sites/{site_id}/content/{id}", put(update_content))
        .route(
            "/api/v1/sites/{site_id}/content/{id}",
            delete(delete_content),
        )
        .route(
            "/api/v1/sites/{site_id}/content/{id}/publish",
            post(publish_content),
        )
        .route(
            "/api/v1/sites/{site_id}/content/{id}/unpublish",
            post(unpublish_content),
        )
}
