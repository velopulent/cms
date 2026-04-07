use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::middleware::auth::AuthenticatedUser;
use crate::models::site::{
    CreateSite, InviteMember, UpdateMemberRole, UpdateSite,
};
use crate::repository::Repository;

pub async fn list_sites(
    auth: AuthenticatedUser,
    Extension(repository): Extension<Repository>,
) -> Response {
    match repository.site.list_for_user(&auth.user_id).await {
        Ok(sites) => (StatusCode::OK, Json(sites)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn create_site(
    auth: AuthenticatedUser,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<CreateSite>,
) -> Response {
    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Name is required"})),
        )
            .into_response();
    }

    let storage_provider = payload
        .default_storage_provider
        .as_deref()
        .unwrap_or("filesystem");
    if storage_provider != "filesystem" && storage_provider != "s3" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid storage provider. Must be 'filesystem' or 's3'"})),
        )
            .into_response();
    }

    let site_id = Uuid::now_v7().to_string();

    match repository.site.create(&site_id, &payload.name, storage_provider, &auth.user_id).await
    {
        Ok(site) => (StatusCode::CREATED, Json(site)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_site(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    match repository.site.get_by_id(&site_id).await {
        Ok(Some(site)) => (StatusCode::OK, Json(site)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Site not found"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn update_site(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateSite>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    let existing = match repository.site.get_by_id(&site_id).await {
        Ok(Some(site)) => site,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Site not found"})),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let name = payload.name.unwrap_or(existing.name);
    let storage_provider = payload
        .default_storage_provider
        .filter(|v| v == "filesystem" || v == "s3")
        .unwrap_or(existing.default_storage_provider);

    match repository.site.update(&site_id, &name, &storage_provider).await {
        Ok(site) => (StatusCode::OK, Json(site)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete_site(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "owner").await {
        return (status, Json(err)).into_response();
    }

    match repository.site.delete(&site_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn list_members(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    match repository.site.list_members(&site_id).await {
        Ok(members) => (StatusCode::OK, Json(members)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn invite_member(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<InviteMember>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    let valid_roles = ["owner", "admin", "editor", "viewer"];
    if !valid_roles.contains(&payload.role.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid role. Must be owner, admin, editor, or viewer"})),
        )
            .into_response();
    }

    let user_id = match repository.user.find_id_by_username(&payload.username).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let member_id = Uuid::now_v7().to_string();

    match repository.site.add_member(&member_id, &site_id, &user_id, &payload.role).await {
        Ok(member) => (StatusCode::CREATED, Json(member)).into_response(),
        Err(crate::repository::error::RepositoryError::UniqueViolation(_)) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "User is already a member of this site"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn update_member_role(
    auth: AuthenticatedUser,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateMemberRole>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "owner").await {
        return (status, Json(err)).into_response();
    }

    let valid_roles = ["owner", "admin", "editor", "viewer"];
    if !valid_roles.contains(&payload.role.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid role"})),
        )
            .into_response();
    }

    match repository.site.update_member_role(&site_id, &member_user_id, &payload.role).await {
        Ok(Some(member)) => (StatusCode::OK, Json(member)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Member not found"})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn remove_member(
    auth: AuthenticatedUser,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = check_site_access_repo(&repository, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    if member_user_id == auth.user_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Cannot remove yourself from the site"})),
        )
            .into_response();
    }

    match repository.site.remove_member(&site_id, &member_user_id).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Member not found"})),
        )
            .into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

pub async fn check_site_access_repo(
    repository: &Repository,
    user_id: &str,
    site_id: &str,
    min_role: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
    let role_order = |r: &str| match r {
        "owner" => 4,
        "admin" => 3,
        "editor" => 2,
        "viewer" => 1,
        _ => 0,
    };

    let min_level = role_order(min_role);

    let role = repository.user.get_role(user_id, site_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            serde_json::json!({"error": e.to_string()}),
        )
    })?;

    match role {
        Some(r) if role_order(&r) >= min_level => Ok(()),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            serde_json::json!({"error": "Insufficient permissions"}),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "Site not found"}),
        )),
    }
}
