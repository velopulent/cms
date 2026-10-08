use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;

use crate::middleware::auth::{
    AuthContext, RequestContext, require_instance_action, require_site_action, require_user_action, scopes_allow_action,
};
use crate::models::authorization::Action;
use crate::models::site::{CreateSite, InviteMember, UpdateMemberRole, UpdateSite};
use crate::repository::Repository;
use crate::services::Services;

#[derive(Deserialize)]
pub struct MemberPath {
    site_id: String,
    member_user_id: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct PublicSite {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

fn public_site(site: crate::models::site::Site) -> PublicSite {
    PublicSite {
        id: site.id,
        name: site.name,
        created_at: site.created_at,
        updated_at: site.updated_at,
    }
}

// ── Public API: /api/v1/sites ──

#[utoipa::path(
    get,
    path = "/api/v1/sites",
    responses((status = 200, description = "Sites accessible to the token", body = Vec<PublicSite>)),
    security(("access_token" = [])),
    tag = "sites"
)]
#[instrument(skip(services, ctx))]
pub async fn list_public_sites(ctx: AuthContext, Extension(services): Extension<Services>) -> Response {
    let allowed = match &ctx.actor {
        crate::middleware::auth::Actor::ApiKey(key) => scopes_allow_action(&key.scopes, Action::SiteRead),
        crate::middleware::auth::Actor::PersonalToken(token) => scopes_allow_action(&token.scopes, Action::SiteRead),
        crate::middleware::auth::Actor::User(_) => false,
    };
    if !allowed {
        return crate::middleware::error::AuthError::insufficient_permission("site.read").into_response();
    }

    match services.site.list_sites_for_actor(&ctx.actor).await {
        Ok(sites) => {
            let public = sites
                .into_iter()
                .filter_map(|site| {
                    Some(PublicSite {
                        id: site.get("id")?.as_str()?.to_owned(),
                        name: site.get("name")?.as_str()?.to_owned(),
                        created_at: site.get("created_at")?.as_str()?.to_owned(),
                        updated_at: site.get("updated_at")?.as_str()?.to_owned(),
                    })
                })
                .collect::<Vec<_>>();
            (StatusCode::OK, Json(public)).into_response()
        }
        Err(error) => error.into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}",
    responses(
        (status = 200, description = "Current site information"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Site not found"),
    ),
    security(("bearer" = []), ("access_token" = [])),
    tag = "site"
)]
#[instrument(skip(repository, services, ctx))]
pub async fn get_current_site(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SiteRead).await {
        return (status, err).into_response();
    }
    match services.site.get_site(&ctx.site_id).await {
        Ok(Some(site)) => (StatusCode::OK, Json(public_site(site))).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

// ── Dashboard: /api/dashboard/sites ──

#[instrument(skip(services, ctx))]
pub async fn list_sites(ctx: AuthContext, Extension(services): Extension<Services>) -> Response {
    match services.site.list_sites_for_actor(&ctx.actor).await {
        Ok(sites) => (StatusCode::OK, Json(sites)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
pub async fn create_site(
    ctx: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<CreateSite>,
) -> Response {
    let created_by = match require_instance_action(&ctx, &repository, Action::SiteCreate).await {
        Ok(user_id) => user_id,
        Err((status, error)) => return (status, error).into_response(),
    };

    match services
        .site
        .create_site(&payload.name, payload.storage_profile_id.as_deref(), &created_by)
        .await
    {
        Ok(site) => (StatusCode::CREATED, Json(site)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
pub async fn get_site(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SiteRead).await {
        return (status, err).into_response();
    }

    match services.site.get_site(&ctx.site_id).await {
        Ok(Some(site)) => (StatusCode::OK, Json(site)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Site not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
pub async fn update_site(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateSite>,
) -> Response {
    if let Err((status, err)) = require_site_action(&ctx, &repository, Action::SiteManage).await {
        return (status, err).into_response();
    }

    match services.site.update_site(&ctx.site_id, payload.name.as_deref()).await {
        Ok(site) => (StatusCode::OK, Json(site)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
pub async fn delete_site(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_instance_action(&ctx.auth, &repository, Action::SiteDelete).await {
        return (status, err).into_response();
    }

    match services.site.delete_site(&ctx.site_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

// ── Dashboard: /api/dashboard/sites/{site_id}/members ──

#[instrument(skip(repository, services, ctx))]
pub async fn list_members(
    ctx: RequestContext,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    if let Err((status, err)) = require_user_action(&ctx, &repository, Action::MembersRead).await {
        return (status, err).into_response();
    }

    match services.site.list_members(&site_id).await {
        Ok(members) => (StatusCode::OK, Json(members)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
pub async fn invite_member(
    ctx: RequestContext,
    Path(site_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<InviteMember>,
) -> Response {
    let actor_id = match require_user_action(&ctx, &repository, Action::MembersManage).await {
        Ok(actor_id) => actor_id,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .site
        .invite_member(&site_id, &payload.email, &payload.role, &actor_id)
        .await
    {
        Ok(member) => (StatusCode::CREATED, Json(member)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx, payload))]
pub async fn update_member_role(
    ctx: RequestContext,
    Path(MemberPath {
        site_id,
        member_user_id,
    }): Path<MemberPath>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
    Json(payload): Json<UpdateMemberRole>,
) -> Response {
    let actor_id = match require_user_action(&ctx, &repository, Action::MembersManage).await {
        Ok(actor_id) => actor_id,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .site
        .update_member_role(&site_id, &member_user_id, &payload.role, &actor_id)
        .await
    {
        Ok(Some(member)) => (StatusCode::OK, Json(member)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Member not found"}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[instrument(skip(repository, services, ctx))]
pub async fn remove_member(
    ctx: RequestContext,
    Path(MemberPath {
        site_id,
        member_user_id,
    }): Path<MemberPath>,
    Extension(repository): Extension<Repository>,
    Extension(services): Extension<Services>,
) -> Response {
    let by_user_id = match require_user_action(&ctx, &repository, Action::MembersManage).await {
        Ok(user_id) => user_id,
        Err((status, err)) => return (status, err).into_response(),
    };

    match services
        .site
        .remove_member(&site_id, &member_user_id, &by_user_id)
        .await
    {
        Ok(0) => (StatusCode::NOT_FOUND, Json(json!({"error": "Member not found"}))).into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
