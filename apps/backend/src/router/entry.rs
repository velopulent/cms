use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};

use crate::handlers::entry_handler::{
    create_collection_entry, create_entry, delete_entry, get_entry, get_entry_revision, get_public_entry,
    list_collection_entries, list_entries, list_entry_revisions, publish_entry, publish_public_entry,
    restore_entry_revision, restore_public_entry_revision, unpublish_entry, unpublish_public_entry, update_entry,
    update_public_entry,
};

pub fn public_routes() -> Router {
    Router::new()
        .route(
            "/collections/{collection_slug}/entries",
            get(list_collection_entries).post(create_collection_entry),
        )
        .route("/entries/{id}", get(get_public_entry))
        .route("/entries/{id}", patch(update_public_entry))
        .route("/entries/{id}", delete(delete_entry))
        .route("/entries/{id}/publish", post(publish_public_entry))
        .route("/entries/{id}/unpublish", post(unpublish_public_entry))
        .route("/entries/{id}/revisions", get(list_entry_revisions))
        .route("/entries/{id}/revisions/{number}", get(get_entry_revision))
        .route(
            "/entries/{id}/revisions/{number}/restore",
            post(restore_public_entry_revision),
        )
}

pub fn dashboard_routes() -> Router {
    Router::new()
        .route("/entries", get(list_entries))
        .route("/entries", post(create_entry))
        .route("/entries/{id}", get(get_entry))
        .route("/entries/{id}", put(update_entry))
        .route("/entries/{id}", delete(delete_entry))
        .route("/entries/{id}/publish", post(publish_entry))
        .route("/entries/{id}/unpublish", post(unpublish_entry))
        .route("/entries/{id}/revisions", get(list_entry_revisions))
        .route("/entries/{id}/revisions/{number}", get(get_entry_revision))
        .route("/entries/{id}/revisions/{number}/restore", post(restore_entry_revision))
}
