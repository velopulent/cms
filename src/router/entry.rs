use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::entry_handler::{
    create_entry, delete_entry, get_entry, list_entries, publish_entry, unpublish_entry, update_entry,
};

pub fn entry_routes() -> Router {
    Router::new()
        .route("/api/v1/sites/{site_id}/entries", get(list_entries))
        .route("/api/v1/sites/{site_id}/entries", post(create_entry))
        .route("/api/v1/sites/{site_id}/entries/{id}", get(get_entry))
        .route("/api/v1/sites/{site_id}/entries/{id}", put(update_entry))
        .route("/api/v1/sites/{site_id}/entries/{id}", delete(delete_entry))
        .route("/api/v1/sites/{site_id}/entries/{id}/publish", post(publish_entry))
        .route("/api/v1/sites/{site_id}/entries/{id}/unpublish", post(unpublish_entry))
}
