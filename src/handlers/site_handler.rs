use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::middleware::auth::{AuthenticatedUser, check_site_access};
use crate::models::site::{
    CreateSite, InviteMember, Site, SiteMember, SiteWithRole, UpdateMemberRole, UpdateSite,
};

#[utoipa::path(
    get,
    path = "/api/v1/sites",
    responses(
        (status = 200, description = "List of sites", body = Vec<SiteWithRole>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "sites"
)]
pub async fn list_sites(
    auth: AuthenticatedUser,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    let result = sqlx::query_as::<_, SiteWithRole>(
        "SELECT s.id, s.name, s.default_storage_provider, s.created_by, s.created_at, s.updated_at, sm.role
         FROM sites s
         JOIN site_members sm ON s.id = sm.site_id
         WHERE sm.user_id = ?
         ORDER BY s.name",
    )
    .bind(&auth.user_id)
    .fetch_all(&pool)
    .await;

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
    path = "/api/v1/sites",
    request_body = CreateSite,
    responses(
        (status = 201, description = "Site created", body = Site),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "sites"
)]
pub async fn create_site(
    auth: AuthenticatedUser,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<CreateSite>,
) -> Response {
    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Name is required"})),
        )
            .into_response();
    }

    let site_id = Uuid::now_v7().to_string();
    let member_id = Uuid::now_v7().to_string();

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let storage_provider = payload.default_storage_provider.as_deref().unwrap_or("filesystem");
    if storage_provider != "filesystem" && storage_provider != "s3" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid storage provider. Must be 'filesystem' or 's3'"})),
        )
            .into_response();
    }

    let result = sqlx::query(
        "INSERT INTO sites (id, name, default_storage_provider, created_by) VALUES (?, ?, ?, ?)",
    )
    .bind(&site_id)
    .bind(&payload.name)
    .bind(&storage_provider)
    .bind(&auth.user_id)
    .execute(&mut *tx)
    .await;

    if let Err(err) = result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response();
    }

    let member_result = sqlx::query(
        "INSERT INTO site_members (id, site_id, user_id, role) VALUES (?, ?, ?, 'owner')",
    )
    .bind(&member_id)
    .bind(&site_id)
    .bind(&auth.user_id)
    .execute(&mut *tx)
    .await;

    if let Err(err) = member_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response();
    }

    if let Err(err) = tx.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response();
    }

    let site = sqlx::query_as::<_, Site>(
        "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
    )
    .bind(&site_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    (StatusCode::CREATED, Json(site)).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 200, description = "Site details", body = Site),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Site not found"),
    ),
    security(("bearer" = []), ("api_key" = [])),
    tag = "sites"
)]
pub async fn get_site(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query_as::<_, Site>(
        "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
    )
    .bind(&site_id)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(site) => (StatusCode::OK, Json(site)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site ID")),
    request_body = UpdateSite,
    responses(
        (status = 200, description = "Site updated", body = Site),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "sites"
)]
pub async fn update_site(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateSite>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    let existing = sqlx::query_as::<_, Site>(
        "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
    )
    .bind(&site_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let name = payload.name.unwrap_or(existing.name);
    let storage_provider = payload.default_storage_provider
        .filter(|v| v == "filesystem" || v == "s3")
        .unwrap_or(existing.default_storage_provider);

    let result = sqlx::query(
        "UPDATE sites SET name = ?, default_storage_provider = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&name)
    .bind(&storage_provider)
    .bind(&site_id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let site = sqlx::query_as::<_, Site>(
                "SELECT id, name, default_storage_provider, created_by, created_at, updated_at FROM sites WHERE id = ?",
            )
            .bind(&site_id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(site)).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 204, description = "Site deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "sites"
)]
pub async fn delete_site(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "owner").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query("DELETE FROM sites WHERE id = ?")
        .bind(&site_id)
        .execute(&pool)
        .await;

    match result {
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
    path = "/api/v1/sites/{site_id}/members",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 200, description = "List of members", body = Vec<SiteMember>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "members"
)]
pub async fn list_members(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "viewer").await {
        return (status, Json(err)).into_response();
    }

    let result = sqlx::query_as::<_, SiteMember>(
        "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
         FROM site_members sm
         JOIN users u ON sm.user_id = u.id
         WHERE sm.site_id = ?
         ORDER BY sm.role DESC, u.username",
    )
    .bind(&site_id)
    .fetch_all(&pool)
    .await;

    match result {
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
    path = "/api/v1/sites/{site_id}/members",
    params(("site_id" = String, Path, description = "Site ID")),
    request_body = InviteMember,
    responses(
        (status = 201, description = "Member added", body = SiteMember),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "members"
)]
pub async fn invite_member(
    auth: AuthenticatedUser,
    Path(site_id): Path<String>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<InviteMember>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "admin").await {
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

    let user: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(&payload.username)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

    let user_id = match user {
        Some((id,)) => id,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
                .into_response()
        }
    };

    let member_id = Uuid::now_v7().to_string();
    let result = sqlx::query(
        "INSERT INTO site_members (id, site_id, user_id, role) VALUES (?, ?, ?, ?)",
    )
    .bind(&member_id)
    .bind(&site_id)
    .bind(&user_id)
    .bind(&payload.role)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            let member = sqlx::query_as::<_, SiteMember>(
                "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
                 FROM site_members sm JOIN users u ON sm.user_id = u.id WHERE sm.id = ?",
            )
            .bind(&member_id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::CREATED, Json(member)).into_response()
        }
        Err(sqlx::Error::Database(ref db_err)) if db_err.is_unique_violation() => (
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
    path = "/api/v1/sites/{site_id}/members/{user_id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("user_id" = String, Path, description = "User ID"),
    ),
    request_body = UpdateMemberRole,
    responses(
        (status = 200, description = "Role updated", body = SiteMember),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "members"
)]
pub async fn update_member_role(
    auth: AuthenticatedUser,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
    Json(payload): Json<UpdateMemberRole>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "owner").await {
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

    let result = sqlx::query(
        "UPDATE site_members SET role = ? WHERE site_id = ? AND user_id = ?",
    )
    .bind(&payload.role)
    .bind(&site_id)
    .bind(&member_user_id)
    .execute(&pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Member not found"})),
        )
            .into_response(),
        Ok(_) => {
            let member = sqlx::query_as::<_, SiteMember>(
                "SELECT sm.id, sm.site_id, sm.user_id, u.username, u.email, sm.role, sm.created_at
                 FROM site_members sm JOIN users u ON sm.user_id = u.id
                 WHERE sm.site_id = ? AND sm.user_id = ?",
            )
            .bind(&site_id)
            .bind(&member_user_id)
            .fetch_one(&pool)
            .await
            .unwrap();

            (StatusCode::OK, Json(member)).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": err.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/sites/{site_id}/members/{user_id}",
    params(
        ("site_id" = String, Path, description = "Site ID"),
        ("user_id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 204, description = "Member removed"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions"),
    ),
    security(("bearer" = [])),
    tag = "members"
)]
pub async fn remove_member(
    auth: AuthenticatedUser,
    Path((site_id, member_user_id)): Path<(String, String)>,
    Extension(pool): Extension<SqlitePool>,
) -> Response {
    if let Err((status, err)) = check_site_access(&pool, &auth.user_id, &site_id, "admin").await {
        return (status, Json(err)).into_response();
    }

    if member_user_id == auth.user_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Cannot remove yourself from the site"})),
        )
            .into_response();
    }

    let result = sqlx::query("DELETE FROM site_members WHERE site_id = ? AND user_id = ?")
        .bind(&site_id)
        .bind(&member_user_id)
        .execute(&pool)
        .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
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
