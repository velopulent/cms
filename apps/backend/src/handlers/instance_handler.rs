use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::middleware::auth::{AuthContext, require_instance_action};
use crate::models::authorization::Action;
use crate::models::user::{CreateManagedUser, UpdateInstanceRole};
use crate::repository::Repository;
use crate::services::Services;

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
            &payload.username,
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
