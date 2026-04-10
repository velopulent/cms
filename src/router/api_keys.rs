use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::handlers::api_key_handler::{create_api_key, delete_api_key, list_api_keys};

pub fn api_key_routes() -> Router {
    Router::new()
        .route("/api/v1/sites/{site_id}/api-keys", get(list_api_keys))
        .route("/api/v1/sites/{site_id}/api-keys", post(create_api_key))
        .route("/api/v1/sites/{site_id}/api-keys/{key_id}", delete(delete_api_key))
}
