use axum::extract::DefaultBodyLimit;
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use tower_http::limit::RequestBodyLimitLayer;

use crate::handlers::file_handler::{
    batch_delete_files, batch_permanent_delete_files, batch_restore_files, delete_file_handler, get_file,
    get_file_references, list_files, restore_file, serve_file, serve_file_thumbnail, upload_file,
    upload_via_signed_url,
};

/// Public API CRUD routes (mounted at /api/v1)
pub fn public_routes(max_upload_bytes: usize) -> Router {
    Router::new()
        .route("/files", get(list_files))
        .merge(
            Router::new()
                .route("/files", post(upload_file))
                .layer(DefaultBodyLimit::disable())
                .layer(RequestBodyLimitLayer::new(max_upload_bytes)),
        )
        .route("/files/batch-delete", post(batch_delete_files))
        .route("/files/batch-restore", post(batch_restore_files))
        .route("/files/batch-permanent-delete", post(batch_permanent_delete_files))
        .route("/files/{id}", get(get_file))
        .route("/files/{id}", delete(delete_file_handler))
        .route("/files/{id}/references", get(get_file_references))
        .route("/files/{id}/restore", post(restore_file))
}

/// File content serving — standalone, no auth middleware
pub fn file_serve_routes() -> Router {
    Router::new()
        .route("/api/files/{id}", get(serve_file))
        .route("/api/files/{id}/thumbnail", get(serve_file_thumbnail))
}

/// Signed-URL upload — standalone, no auth middleware: the HMAC token in the
/// path is the credential (minted by the MCP `create_upload_url` tool).
/// Registered at the literal path the tool advertises.
pub fn signed_upload_routes(max_upload_bytes: usize) -> Router {
    Router::new()
        .route("/api/v1/files/upload/{token}", put(upload_via_signed_url))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(max_upload_bytes))
}

/// Dashboard routes (mounted under /api/dashboard/sites/{site_id})
pub fn dashboard_routes(max_upload_bytes: usize) -> Router {
    Router::new()
        .route("/files", get(list_files))
        .merge(
            Router::new()
                .route("/files", post(upload_file))
                .layer(DefaultBodyLimit::disable())
                .layer(RequestBodyLimitLayer::new(max_upload_bytes)),
        )
        .route("/files/batch-delete", post(batch_delete_files))
        .route("/files/batch-restore", post(batch_restore_files))
        .route("/files/batch-permanent-delete", post(batch_permanent_delete_files))
        .route("/files/{id}", get(get_file))
        .route("/files/{id}", delete(delete_file_handler))
        .route("/files/{id}/references", get(get_file_references))
        .route("/files/{id}/restore", post(restore_file))
}
