use axum::{
    routing::get,
    Router,
};

use crate::handlers::ui_handler::ui_handler;

pub fn dashboard_routes() -> Router {
    Router::new().route(
            "/dashboard",
            get(|| async { ui_handler(axum::extract::Path("".into())).await }),
        )
        .route("/dashboard/{*file}", get(ui_handler))
}
