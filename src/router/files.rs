use axum::extract::DefaultBodyLimit;
use axum::{
    Router,
    routing::{delete, get, post},
};
use tower_http::limit::RequestBodyLimitLayer;

use crate::handlers::file_handler::{
    batch_delete_files, batch_permanent_delete_files, batch_restore_files, delete_file_handler, get_file,
    get_file_references, list_files, restore_file, serve_file, serve_file_thumbnail, upload_file,
};

pub fn file_routes(max_upload_bytes: usize) -> Router {
    Router::new()
        .route("/api/v1/files", get(list_files))
        .merge(
            Router::new()
                .route("/api/v1/files", post(upload_file))
                .layer(DefaultBodyLimit::disable())
                .layer(RequestBodyLimitLayer::new(max_upload_bytes)),
        )
        .route("/api/v1/files/batch-delete", post(batch_delete_files))
        .route("/api/v1/files/batch-restore", post(batch_restore_files))
        .route(
            "/api/v1/files/batch-permanent-delete",
            post(batch_permanent_delete_files),
        )
        .route("/api/v1/files/{id}", get(get_file))
        .route("/api/v1/files/{id}", delete(delete_file_handler))
        .route("/api/v1/files/{id}/references", get(get_file_references))
        .route("/api/v1/files/{id}/restore", post(restore_file))
        .route("/api/files/{id}", get(serve_file))
        .route("/api/files/{id}/thumbnail", get(serve_file_thumbnail))
}
