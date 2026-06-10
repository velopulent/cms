use axum::{
    Router,
    routing::{get, post, put},
};

use crate::handlers::instance_handler::{create_user, list_users, update_instance_role};

pub fn routes() -> Router {
    Router::new()
        .route("/instance/users", get(list_users))
        .route("/instance/users", post(create_user))
        .route("/instance/users/{user_id}/role", put(update_instance_role))
}
