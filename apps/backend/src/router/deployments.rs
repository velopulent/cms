use crate::handlers::deployment_handler;
use axum::{
    Router,
    routing::{get, post},
};
pub fn routes() -> Router {
    Router::new()
        .route(
            "/deployments",
            get(deployment_handler::list).post(deployment_handler::create),
        )
        .route(
            "/deployments/{trigger_id}",
            axum::routing::put(deployment_handler::update).delete(deployment_handler::delete),
        )
        .route("/deployments/{trigger_id}/trigger", post(deployment_handler::trigger))
        .route("/deployments/{trigger_id}/history", get(deployment_handler::history))
}
