use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use crate::middleware::auth::{
    Principal, SCOPE_MEMBERS_READ, SCOPE_MEMBERS_WRITE, SCOPE_SITES_DELETE, SCOPE_SITES_READ, SCOPE_SITES_WRITE,
    require_admin_scope,
};
use crate::models::site::{CreateSite, InviteMember, Site, SiteMember, SiteWithRole, UpdateMemberRole, UpdateSite};
use crate::repository::Repository;
use crate::repository::error::RepositoryError;

#[utoipa::path(
    get,
    path = "/api/v1/admin/sites",
    responses(
        (status = 200, description = "List sites visible to the caller", body = Vec<SiteWithRole>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(repository, principal))]
pub async fn list_sites(principal: Principal, Extension(repository): Extension<Repository>) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_SITES_READ).await {
        return (status, Json(err)).into_response();
    }

    let result = match &principal {
        Principal::InstanceToken { .. } => repository.site.list_all().await.map(|sites| {
            sites.into_iter()
                .map(|site| {
                    serde_json::json!({
                        "id": site.id,
                        "name": site.name,
                        "default_storage_provider": site.default_storage_provider,
                        "created_by": site.created_by,
                        "created_at": site.created_at,
                        "updated_at": site.updated_at,
                        "role": "instance_admin",
                    })
                })
                .collect::<Vec<_>>()
        }),
        Principal::UserSession { user_id } => repository.site.list_for_user(user_id).await.map(|sites| {
            sites.into_iter()
                .map(|site| serde_json::to_value(site).unwrap_or_default())
                .collect::<Vec<_>>()
        }),
        Principal::SiteToken { .. } => unreachable!(),
    };

    match result {
        Ok(sites) => (StatusCode::OK, Json(sites)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/sites",
    request_body = CreateSite,
    responses(
        (status = 201, description = "Site created", body = Site),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(repository, principal, payload))]
pub async fn create_site(
    principal: Principal,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<CreateSite>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_SITES_WRITE).await {
        return (status, Json(err)).into_response();
    }

    if payload.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Name is required"}))).into_response();
    }

    let storage_provider = payload.default_storage_provider.as_deref().unwrap_or("filesystem");
    if storage_provider != "filesystem" && storage_provider != "s3" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid storage provider. Must be 'filesystem' or 's3'"})),
        )
            .into_response();
    }

    let site_id = Uuid::now_v7().to_string();

    let created_by = principal.user_id().unwrap_or("system");

    match repository
        .site
        .create(&site_id, &payload.name, storage_provider, created_by)
        .await
    {
        Ok(site) => (StatusCode::CREATED, Json(site)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site id")),
    responses(
        (status = 200, description = "Site details", body = Site),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Site not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(repository, principal))]
pub async fn get_site(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_SITES_READ).await {
        return (status, Json(err)).into_response();
    }

    match repository.site.get_by_id(&site_id).await {
        Ok(Some(site)) => (StatusCode::OK, Json(site)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site id")),
    request_body = UpdateSite,
    responses(
        (status = 200, description = "Site updated", body = Site),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Site not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(repository, principal, payload))]
pub async fn update_site(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateSite>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_SITES_WRITE).await {
        return (status, Json(err)).into_response();
    }

    let existing = match repository.site.get_by_id(&site_id).await {
        Ok(Some(site)) => site,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response();
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

#[utoipa::path(
    delete,
    path = "/api/v1/admin/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site id")),
    responses(
        (status = 204, description = "Site deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(repository, principal))]
pub async fn delete_site(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_SITES_DELETE).await {
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

#[utoipa::path(
    get,
    path = "/api/v1/admin/sites/{site_id}/members",
    params(("site_id" = String, Path, description = "Site id")),
    responses(
        (status = 200, description = "List site members", body = Vec<SiteMember>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-members"
)]
#[instrument(skip(repository, principal))]
pub async fn list_members(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_READ).await {
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

#[utoipa::path(
    post,
    path = "/api/v1/admin/sites/{site_id}/members",
    params(("site_id" = String, Path, description = "Site id")),
    request_body = InviteMember,
    responses(
        (status = 201, description = "Member invited", body = SiteMember),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "User not found"),
        (status = 409, description = "User already a member"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-members"
)]
#[instrument(skip(repository, principal, payload))]
pub async fn invite_member(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<InviteMember>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_WRITE).await {
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
            return (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))).into_response();
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

    match repository
        .site
        .add_member(&member_id, &site_id, &user_id, &payload.role)
        .await
    {
        Ok(member) => (StatusCode::CREATED, Json(member)).into_response(),
        Err(RepositoryError::UniqueViolation(_)) => (
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

#[utoipa::path(
    put,
    path = "/api/v1/admin/sites/{site_id}/members/{user_id}",
    params(
        ("site_id" = String, Path, description = "Site id"),
        ("user_id" = String, Path, description = "User id")
    ),
    request_body = UpdateMemberRole,
    responses(
        (status = 200, description = "Member role updated", body = SiteMember),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Member not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-members"
)]
#[instrument(skip(repository, principal, payload))]
pub async fn update_member_role(
    principal: Principal,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Json(payload): Json<UpdateMemberRole>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_WRITE).await {
        return (status, Json(err)).into_response();
    }

    let valid_roles = ["owner", "admin", "editor", "viewer"];
    if !valid_roles.contains(&payload.role.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid role"}))).into_response();
    }

    match repository
        .site
        .update_member_role(&site_id, &member_user_id, &payload.role)
        .await
    {
        Ok(Some(member)) => (StatusCode::OK, Json(member)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Member not found"}))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/sites/{site_id}/members/{user_id}",
    params(
        ("site_id" = String, Path, description = "Site id"),
        ("user_id" = String, Path, description = "User id")
    ),
    responses(
        (status = 204, description = "Member removed"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Member not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-members"
)]
#[instrument(skip(repository, principal))]
pub async fn remove_member(
    principal: Principal,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_WRITE).await {
        return (status, Json(err)).into_response();
    }

    if principal.user_id().is_some_and(|user_id| member_user_id == user_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Cannot remove yourself from the site"})),
        )
            .into_response();
    }

    match repository.site.remove_member(&site_id, &member_user_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Member not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}
