use axum::{routing::post, Router};

use crate::handlers::auth_handler::{login, me, register};

pub fn auth_routes() -> Router {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", axum::routing::get(me))
}
