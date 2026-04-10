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
        .route("/api/v1/sites/{site_id}/files", get(list_files))
        // Upload route uses a nested router to disable DefaultBodyLimit
        // before applying RequestBodyLimitLayer (avoids type inference issue
        // with MethodRouter::layer)
        .merge(
            Router::new()
                .route("/api/v1/sites/{site_id}/files", post(upload_file))
                .layer(DefaultBodyLimit::disable())
                .layer(RequestBodyLimitLayer::new(max_upload_bytes)),
        )
        .route("/api/v1/sites/{site_id}/files/batch-delete", post(batch_delete_files))
        .route("/api/v1/sites/{site_id}/files/batch-restore", post(batch_restore_files))
        .route(
            "/api/v1/sites/{site_id}/files/batch-permanent-delete",
            post(batch_permanent_delete_files),
        )
        .route("/api/v1/sites/{site_id}/files/{id}", get(get_file))
        .route("/api/v1/sites/{site_id}/files/{id}", delete(delete_file_handler))
        .route(
            "/api/v1/sites/{site_id}/files/{id}/references",
            get(get_file_references),
        )
        .route("/api/v1/sites/{site_id}/files/{id}/restore", post(restore_file))
        // File serving (public, no auth)
        .route("/api/files/{id}", get(serve_file))
        .route("/api/files/{id}/thumbnail", get(serve_file_thumbnail))
}
