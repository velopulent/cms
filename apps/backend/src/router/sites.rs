use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::site_handler::{
    create_site, delete_site, get_site, invite_member, list_members, list_sites, remove_member, transfer_ownership,
    update_member_role, update_site,
};

/// Dashboard routes at /sites (nested under /api/dashboard)
pub fn dashboard_list_routes() -> Router {
    Router::new()
        .route("/sites", get(list_sites))
        .route("/sites", post(create_site))
}

/// Dashboard site-scoped routes (nested under /api/dashboard/sites/{site_id})
pub fn dashboard_site_routes() -> Router {
    Router::new()
        .route("/", get(get_site))
        .route("/", put(update_site))
        .route("/", delete(delete_site))
        .route("/members", get(list_members))
        .route("/members", post(invite_member))
        .route("/members/{user_id}", put(update_member_role))
        .route("/members/{user_id}", delete(remove_member))
        .route("/ownership", post(transfer_ownership))
}
