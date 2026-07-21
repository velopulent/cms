use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::handlers::instance_handler::{
    create_user, delete_user, list_users, set_user_password, update_instance_role, update_user,
};
use crate::handlers::settings_handler::{
    get_settings, update_backups, update_general, update_security, update_storage,
};

pub fn routes() -> Router {
    Router::new()
        .route("/instance/users", get(list_users))
        .route("/instance/users", post(create_user))
        .route("/instance/users/{user_id}", put(update_user))
        .route("/instance/users/{user_id}", delete(delete_user))
        .route("/instance/users/{user_id}/role", put(update_instance_role))
        .route("/instance/users/{user_id}/password", post(set_user_password))
        .route("/instance/settings", get(get_settings))
        .route("/instance/settings/general", put(update_general))
        .route("/instance/settings/security", put(update_security))
        .route("/instance/settings/storage", put(update_storage))
        .route("/instance/settings/backups", put(update_backups))
}
