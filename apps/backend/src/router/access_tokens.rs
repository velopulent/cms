use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::handlers::access_token_handler::{create_personal_token, list_personal_tokens, revoke_personal_token};
use crate::handlers::access_token_handler::{create_site_token, delete_site_token, list_site_tokens};

pub fn dashboard_routes() -> Router {
    Router::new()
        .route("/tokens", get(list_site_tokens))
        .route("/tokens", post(create_site_token))
        .route("/tokens/{token_id}", delete(delete_site_token))
}

pub fn account_routes() -> Router {
    Router::new()
        .route("/account/tokens", get(list_personal_tokens).post(create_personal_token))
        .route("/account/tokens/{token_id}", delete(revoke_personal_token))
}
