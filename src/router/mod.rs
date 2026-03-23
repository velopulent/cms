use axum::Extension;
use axum::{
    Router,
    routing::{delete, get, post, put},
};
use sqlx::SqlitePool;

use crate::handlers::auth_handler::{login, me, register};
use crate::handlers::content_handler::{
    create_content, delete_content, get_content, list_content, publish_content, unpublish_content,
    update_content,
};
use crate::handlers::content_type_handler::{
    create_content_type, delete_content_type, get_content_type, list_content_types,
    update_content_type,
};
use crate::handlers::ui_handler::ui_handler;

pub fn create_router(pool: SqlitePool) -> Router {
    Router::new()
        // SPA
        .route(
            "/",
            get(|| async { ui_handler(axum::extract::Path("".into())).await }),
        )
        .route("/{*file}", get(ui_handler))
        // Auth
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        // Content Types
        .route("/api/content-types", get(list_content_types))
        .route("/api/content-types", post(create_content_type))
        .route("/api/content-types/{slug}", get(get_content_type))
        .route("/api/content-types/{id}", put(update_content_type))
        .route("/api/content-types/{id}", delete(delete_content_type))
        // Content
        .route("/api/content", get(list_content))
        .route("/api/content", post(create_content))
        .route("/api/content/{id}", get(get_content))
        .route("/api/content/{id}", put(update_content))
        .route("/api/content/{id}", delete(delete_content))
        .route("/api/content/{id}/publish", post(publish_content))
        .route("/api/content/{id}/unpublish", post(unpublish_content))
        .layer(Extension(pool))
}
