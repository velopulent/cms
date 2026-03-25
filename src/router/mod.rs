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
use crate::handlers::schema_handler::{
    create_schema, delete_schema, get_schema, list_schemas, update_schema,
};
use crate::handlers::site_handler::{
    create_site, delete_site, get_site, invite_member, list_members, list_sites, remove_member,
    update_member_role, update_site,
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
        // Sites
        .route("/api/sites", get(list_sites))
        .route("/api/sites", post(create_site))
        .route("/api/sites/{site_id}", get(get_site))
        .route("/api/sites/{site_id}", put(update_site))
        .route("/api/sites/{site_id}", delete(delete_site))
        // Site Members
        .route("/api/sites/{site_id}/members", get(list_members))
        .route("/api/sites/{site_id}/members", post(invite_member))
        .route(
            "/api/sites/{site_id}/members/{user_id}",
            put(update_member_role),
        )
        .route(
            "/api/sites/{site_id}/members/{user_id}",
            delete(remove_member),
        )
        // Schemas (site-scoped)
        .route(
            "/api/sites/{site_id}/schemas",
            get(list_schemas),
        )
        .route(
            "/api/sites/{site_id}/schemas",
            post(create_schema),
        )
        .route(
            "/api/sites/{site_id}/schemas/{schema_slug}",
            get(get_schema),
        )
        .route(
            "/api/sites/{site_id}/schemas/{schema_slug}",
            put(update_schema),
        )
        .route(
            "/api/sites/{site_id}/schemas/{schema_slug}",
            delete(delete_schema),
        )
        // Content (site-scoped)
        .route("/api/sites/{site_id}/content", get(list_content))
        .route("/api/sites/{site_id}/content", post(create_content))
        .route("/api/sites/{site_id}/content/{id}", get(get_content))
        .route("/api/sites/{site_id}/content/{id}", put(update_content))
        .route("/api/sites/{site_id}/content/{id}", delete(delete_content))
        .route(
            "/api/sites/{site_id}/content/{id}/publish",
            post(publish_content),
        )
        .route(
            "/api/sites/{site_id}/content/{id}/unpublish",
            post(unpublish_content),
        )
        .layer(Extension(pool))
}
