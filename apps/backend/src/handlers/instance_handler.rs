use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::middleware::auth::{Actor, AuthContext, require_instance_action};
use crate::models::authorization::Action;
use crate::models::user::{AdminSetPassword, CreateManagedUser, UpdateInstanceRole, UpdateUserProfile};
use crate::repository::Repository;
use crate::repository::error::RepositoryError;
use crate::services::Services;

/// True when the target user currently holds the instance-owner role. Editing or deleting
/// an owner is owner-only (`InstanceRolesGrant`); everything else needs `InstanceManage`.
/// Errors propagate so a DB failure fails closed (denies) instead of silently reporting
/// "not an owner" and dropping to the weaker `InstanceManage` gate.
async fn target_is_owner(repository: &Repository, user_id: &str) -> Result<bool, RepositoryError> {
    Ok(repository
        .user
        .find_by_id(user_id)
        .await?
        .map(|user| user.instance_role.as_deref() == Some("instance_owner"))
        .unwrap_or(false))
}

/// Shared 500 response when the owner check itself fails (DB error).
fn owner_check_failed() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": "Failed to verify target user" })),
    )
        .into_response()
}

pub async fn list_users(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceManage).await {
        return (status, error).into_response();
    }
    match services.auth.list_users().await {
        Ok(users) => (StatusCode::OK, Json(users)).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn create_user(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateManagedUser>,
) -> Response {
    if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceManage).await {
        return (status, error).into_response();
    }
    // Only an instance owner may mint another instance owner; admins may grant instance_admin.
    if payload.instance_role.as_deref() == Some("instance_owner")
        && let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
    {
        return (status, error).into_response();
    }
    match services
        .auth
        .create_managed_user(
            &payload.name,
            &payload.email,
            &payload.temporary_password,
            payload.instance_role.as_deref(),
        )
        .await
    {
        Ok(user) => (StatusCode::CREATED, Json(user)).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn update_instance_role(
    auth: AuthContext,
    Path(user_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateInstanceRole>,
) -> Response {
    if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceManage).await {
        return (status, error).into_response();
    }
    // Anything that grants or revokes the owner role is owner-only.
    let target_owner = match target_is_owner(&repository, &user_id).await {
        Ok(value) => value,
        Err(_) => return owner_check_failed(),
    };
    if (payload.instance_role.as_deref() == Some("instance_owner") || target_owner)
        && let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
    {
        return (status, error).into_response();
    }
    match services
        .auth
        .set_instance_role(&user_id, payload.instance_role.as_deref())
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn update_user(
    auth: AuthContext,
    Path(user_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateUserProfile>,
) -> Response {
    if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceManage).await {
        return (status, error).into_response();
    }
    match target_is_owner(&repository, &user_id).await {
        Ok(true) => {
            if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
            {
                return (status, error).into_response();
            }
        }
        Ok(false) => {}
        Err(_) => return owner_check_failed(),
    }
    match services
        .auth
        .update_user_profile(&user_id, &payload.name, &payload.email)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn set_user_password(
    auth: AuthContext,
    Path(user_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<AdminSetPassword>,
) -> Response {
    if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceManage).await {
        return (status, error).into_response();
    }
    match target_is_owner(&repository, &user_id).await {
        Ok(true) => {
            if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
            {
                return (status, error).into_response();
            }
        }
        Ok(false) => {}
        Err(_) => return owner_check_failed(),
    }
    match services.auth.admin_set_password(&user_id, &payload.new_password).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn delete_user(
    auth: AuthContext,
    Path(user_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceManage).await {
        return (status, error).into_response();
    }
    // Operators cannot delete their own account from here.
    if let Actor::User(actor) = &auth.actor
        && actor.user_id == user_id
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "You cannot delete your own account" })),
        )
            .into_response();
    }
    match target_is_owner(&repository, &user_id).await {
        Ok(true) => {
            if let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
            {
                return (status, error).into_response();
            }
        }
        Ok(false) => {}
        Err(_) => return owner_check_failed(),
    }
    match services.auth.delete_user(&user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}
