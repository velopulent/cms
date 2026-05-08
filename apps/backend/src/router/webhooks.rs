use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::webhook_handler::{
    create_webhook, delete_webhook, get_webhook, list_deliveries, list_webhooks, trigger_webhook, update_webhook,
};

pub fn webhook_routes() -> Router {
    Router::new()
        .route("/api/v1/sites/{site_id}/webhooks", get(list_webhooks))
        .route("/api/v1/sites/{site_id}/webhooks", post(create_webhook))
        .route(
            "/api/v1/sites/{site_id}/webhooks/{webhook_id}",
            get(get_webhook),
        )
        .route(
            "/api/v1/sites/{site_id}/webhooks/{webhook_id}",
            put(update_webhook),
        )
        .route(
            "/api/v1/sites/{site_id}/webhooks/{webhook_id}",
            delete(delete_webhook),
        )
        .route(
            "/api/v1/sites/{site_id}/webhooks/{webhook_id}/trigger",
            post(trigger_webhook),
        )
        .route(
            "/api/v1/sites/{site_id}/webhooks/{webhook_id}/deliveries",
            get(list_deliveries),
        )
}