use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{
    Principal, SCOPE_MEMBERS_READ, SCOPE_MEMBERS_WRITE, SCOPE_SITES_DELETE, SCOPE_SITES_READ, SCOPE_SITES_WRITE,
    require_admin_scope,
};
use crate::models::site::{CreateSite, InviteMember, Site, SiteMember, UpdateMemberRole, UpdateSite};
use crate::repository::Repository;
use crate::services::Services;

#[utoipa::path(
    get,
    path = "/api/v1/sites",
    responses(
        (status = 200, description = "List sites visible to the caller"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(repository, services, principal))]
pub async fn list_sites(
    principal: Principal,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_SITES_READ).await {
        return (status, err).into_response();
    }

    match services.site.list_sites_for_principal(&principal).await {
        Ok(sites) => (StatusCode::OK, Json(sites)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites",
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
#[instrument(skip(repository, services, principal, payload))]
pub async fn create_site(
    principal: Principal,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateSite>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, None, SCOPE_SITES_WRITE).await {
        return (status, err).into_response();
    }

    let created_by = principal.user_id().unwrap_or("system");

    match services
        .site
        .create_site(&payload.name, payload.default_storage_provider.as_deref(), created_by)
        .await
    {
        Ok(site) => (StatusCode::CREATED, Json(site)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}",
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
#[instrument(skip(repository, services, principal))]
pub async fn get_site(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_SITES_READ).await {
        return (status, err).into_response();
    }

    match services.site.get_site(&site_id).await {
        Ok(Some(site)) => (StatusCode::OK, Json(site)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}",
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
#[instrument(skip(repository, services, principal, payload))]
pub async fn update_site(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateSite>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_SITES_WRITE).await {
        return (status, err).into_response();
    }

    match services
        .site
        .update_site(
            &site_id,
            payload.name.as_deref(),
            payload.default_storage_provider.as_deref(),
        )
        .await
    {
        Ok(site) => (StatusCode::OK, Json(site)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site id")),
    responses(
        (status = 204, description = "Site deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(repository, services, principal))]
pub async fn delete_site(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_SITES_DELETE).await {
        return (status, err).into_response();
    }

    match services.site.delete_site(&site_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}/members",
    params(("site_id" = String, Path, description = "Site id")),
    responses(
        (status = 200, description = "List site members", body = Vec<SiteMember>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site-members"
)]
#[instrument(skip(repository, services, principal))]
pub async fn list_members(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_READ).await {
        return (status, err).into_response();
    }

    match services.site.list_members(&site_id).await {
        Ok(members) => (StatusCode::OK, Json(members)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/sites/{site_id}/members",
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
#[instrument(skip(repository, services, principal, payload))]
pub async fn invite_member(
    principal: Principal,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<InviteMember>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_WRITE).await
    {
        return (status, err).into_response();
    }

    match services
        .site
        .invite_member(&site_id, &payload.username, &payload.role)
        .await
    {
        Ok(member) => (StatusCode::CREATED, Json(member)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}/members/{user_id}",
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
#[instrument(skip(repository, services, principal, payload))]
pub async fn update_member_role(
    principal: Principal,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateMemberRole>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_WRITE).await
    {
        return (status, err).into_response();
    }

    match services
        .site
        .update_member_role(&site_id, &member_user_id, &payload.role)
        .await
    {
        Ok(Some(member)) => (StatusCode::OK, Json(member)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Member not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/members/{user_id}",
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
#[instrument(skip(repository, services, principal))]
pub async fn remove_member(
    principal: Principal,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_admin_scope(&principal, &repository, Some(&site_id), SCOPE_MEMBERS_WRITE).await
    {
        return (status, err).into_response();
    }

    let by_user_id = principal.user_id().unwrap_or("unknown");

    match services.site.remove_member(&site_id, &member_user_id, by_user_id).await {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Member not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
