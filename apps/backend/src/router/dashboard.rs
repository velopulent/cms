use axum::{Router, routing::get};

use crate::handlers::dashboard_handler::dashboard_handler;

pub fn dashboard_routes() -> Router {
    Router::new()
        .route(
            "/dashboard",
            get(|| async { dashboard_handler(axum::extract::Path("".into())).await }),
        )
        // Bare `/dashboard/` (trailing slash) matches neither `/dashboard` nor the
        // `{*file}` wildcard (which needs ≥1 segment); serve the SPA shell here too so a
        // refresh on a client route URL ending in `/` still loads the app.
        .route(
            "/dashboard/",
            get(|| async { dashboard_handler(axum::extract::Path("".into())).await }),
        )
        .route("/dashboard/{*file}", get(dashboard_handler))
}
