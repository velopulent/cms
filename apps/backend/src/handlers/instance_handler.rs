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
use crate::services::Services;

/// True when the target user currently holds the instance-owner role. Editing or deleting
/// an owner is owner-only (`InstanceRolesGrant`); everything else needs `InstanceManage`.
async fn target_is_owner(repository: &Repository, user_id: &str) -> bool {
    matches!(
        repository.user.find_by_id(user_id).await,
        Ok(Some(user)) if user.instance_role.as_deref() == Some("instance_owner")
    )
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
    let target_is_owner = matches!(
        repository.user.find_by_id(&user_id).await,
        Ok(Some(user)) if user.instance_role.as_deref() == Some("instance_owner")
    );
    if (payload.instance_role.as_deref() == Some("instance_owner") || target_is_owner)
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
    if target_is_owner(&repository, &user_id).await
        && let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
    {
        return (status, error).into_response();
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
    if target_is_owner(&repository, &user_id).await
        && let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
    {
        return (status, error).into_response();
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
    if target_is_owner(&repository, &user_id).await
        && let Err((status, error)) = require_instance_action(&auth, &repository, Action::InstanceRolesGrant).await
    {
        return (status, error).into_response();
    }
    match services.auth.delete_user(&user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}
