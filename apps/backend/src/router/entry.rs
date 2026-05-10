use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::entry_handler::{
    create_entry, delete_entry, get_entry, get_entry_revision, list_entries, list_entry_revisions, publish_entry,
    restore_entry_revision, unpublish_entry, update_entry,
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
        .route("/api/v1/entries/{id}/revisions", get(list_entry_revisions))
        .route("/api/v1/entries/{id}/revisions/{number}", get(get_entry_revision))
        .route(
            "/api/v1/entries/{id}/revisions/{number}/restore",
            post(restore_entry_revision),
        )
}
