use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::site_handler::{
    create_site, delete_site, get_site, invite_member, list_members, list_sites, remove_member, update_member_role,
    update_site,
};

pub fn site_routes() -> Router {
    Router::new()
        .route("/api/v1/sites", get(list_sites))
        .route("/api/v1/sites", post(create_site))
        .route("/api/v1/sites/{site_id}", get(get_site))
        .route("/api/v1/sites/{site_id}", put(update_site))
        .route("/api/v1/sites/{site_id}", delete(delete_site))
        .route("/api/v1/sites/{site_id}/members", get(list_members))
        .route("/api/v1/sites/{site_id}/members", post(invite_member))
        .route("/api/v1/sites/{site_id}/members/{user_id}", put(update_member_role))
        .route("/api/v1/sites/{site_id}/members/{user_id}", delete(remove_member))
}
