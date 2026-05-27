use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::webhook_handler::{
    create_webhook, delete_webhook, get_webhook, list_deliveries, list_webhooks, trigger_webhook, update_webhook,
};

pub fn public_routes() -> Router {
    Router::new()
        .route("/webhooks", get(list_webhooks))
        .route("/webhooks", post(create_webhook))
        .route("/webhooks/{webhook_id}", get(get_webhook))
        .route("/webhooks/{webhook_id}", put(update_webhook))
        .route("/webhooks/{webhook_id}", delete(delete_webhook))
        .route("/webhooks/{webhook_id}/trigger", post(trigger_webhook))
        .route("/webhooks/{webhook_id}/deliveries", get(list_deliveries))
}

pub fn dashboard_routes() -> Router {
    Router::new()
        .route("/webhooks", get(list_webhooks))
        .route("/webhooks", post(create_webhook))
        .route("/webhooks/{webhook_id}", get(get_webhook))
        .route("/webhooks/{webhook_id}", put(update_webhook))
        .route("/webhooks/{webhook_id}", delete(delete_webhook))
        .route("/webhooks/{webhook_id}/trigger", post(trigger_webhook))
        .route("/webhooks/{webhook_id}/deliveries", get(list_deliveries))
}
