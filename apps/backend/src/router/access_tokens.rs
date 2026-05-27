use axum::{Router, routing::{delete, get, post}};

use crate::handlers::access_token_handler::{create_site_token, delete_site_token, list_site_tokens};

pub fn dashboard_routes() -> Router {
    Router::new()
        .route("/tokens", get(list_site_tokens))
        .route("/tokens", post(create_site_token))
        .route("/tokens/{token_id}", delete(delete_site_token))
}
