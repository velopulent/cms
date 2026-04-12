use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::handlers::access_token_handler::{
    create_instance_token, create_site_token, delete_instance_token, delete_site_token, list_instance_tokens,
    list_site_tokens,
};

pub fn access_token_routes() -> Router {
    Router::new()
        .route("/api/v1/admin/tokens", get(list_instance_tokens))
        .route("/api/v1/admin/tokens", post(create_instance_token))
        .route("/api/v1/admin/tokens/{token_id}", delete(delete_instance_token))
        .route("/api/v1/site/tokens", get(list_site_tokens))
        .route("/api/v1/site/tokens", post(create_site_token))
        .route("/api/v1/site/tokens/{token_id}", delete(delete_site_token))
}
