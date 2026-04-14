use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::handlers::entry_handler::{
    create_entry, delete_entry, get_entry, list_entries, publish_entry, unpublish_entry, update_entry,
};

pub fn entry_routes() -> Router {
    Router::new()
        .route("/api/v1/entries", get(list_entries))
        .route("/api/v1/entries", post(create_entry))
        .route("/api/v1/entries/{id}", get(get_entry))
        .route("/api/v1/entries/{id}", put(update_entry))
        .route("/api/v1/entries/{id}", delete(delete_entry))
        .route("/api/v1/entries/{id}/publish", post(publish_entry))
        .route("/api/v1/entries/{id}/unpublish", post(unpublish_entry))
}
