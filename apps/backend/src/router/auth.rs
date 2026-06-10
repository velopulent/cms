use axum::{
    Router,
    middleware::from_fn,
    routing::{get, post},
};

use crate::handlers::auth_handler::{change_password, list_sessions, login, logout, me, register, revoke_all_sessions};
use crate::middleware::dashboard_auth::dashboard_auth_middleware;
use crate::middleware::rate_limit::rate_limit_middleware;

pub fn auth_routes() -> Router {
    let protected = Router::new()
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
        .route("/api/auth/sessions", get(list_sessions))
        .route("/api/auth/sessions/revoke-all", post(revoke_all_sessions))
        .route("/api/auth/change-password", post(change_password))
        .layer(from_fn(dashboard_auth_middleware));

    let public = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .layer(from_fn(rate_limit_middleware));

    public.merge(protected)
}
