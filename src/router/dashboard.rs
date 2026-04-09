use axum::{
    routing::get,
    Router,
};

use crate::handlers::dashboard_handler::dashboard_handler;

pub fn dashboard_routes() -> Router {
    Router::new().route(
            "/dashboard",
            get(|| async { dashboard_handler(axum::extract::Path("".into())).await }),
        )
        .route("/dashboard/{*file}", get(dashboard_handler))
}
