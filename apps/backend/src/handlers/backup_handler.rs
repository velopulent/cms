//! HTTP handlers for backup & restore.
//!
//! Instance-scope handlers are owner-gated (`InstanceBackup`/`InstanceRestore`)
//! and live under `/api/dashboard/instance`. Site-scope handlers are operator-gated
//! (`SiteBackup`/`SiteRestore` — editors/viewers/API keys are denied) and live
//! under `/api/dashboard/sites/{site_id}`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::header;
use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::multipart::Multipart;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::{AuthContext, RequestContext, require_instance_action, require_user_action};
use crate::models::authorization::Action;
use crate::repository::Repository;
use crate::services::backup::{
    BackupError, BackupService, CreateBackupOptions, RestoreRequest, RestoreSource, RestoreTarget, Scope, meta, schedule,
};
use crate::storage::StorageRegistry;

// ── Request bodies ──

#[derive(Deserialize)]
pub struct CreateBackupBody {
    #[serde(default)]
    pub include_files: Option<bool>,
    #[serde(default)]
    pub encrypt: bool,
}

#[derive(Deserialize)]
pub struct ScheduleBody {
    pub cron: String,
    pub retention_n: Option<i64>,
    pub include_files: Option<bool>,
    pub encrypt: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct RestoreBody {
    pub backup_id: Option<String>,
    pub destination_key: Option<String>,
    /// "instance" or "site". For site-scope endpoints this is ignored (always site).
    pub mode: Option<String>,
    pub site_id: Option<String>,
    #[serde(default)]
    pub import_as_new: bool,
    pub confirm: Option<String>,
}

#[derive(Deserialize)]
pub struct BackupPath {
    #[allow(dead_code)]
    pub site_id: String,
    pub backup_id: String,
}

#[derive(Deserialize)]
pub struct SchedulePath {
    #[allow(dead_code)]
    pub site_id: String,
    pub schedule_id: String,
}

const RESTORE_CONFIRMATION: &str = "RESTORE";

// ── Error mapping ──

fn err_response(e: BackupError) -> Response {
    let (status, code) = match &e {
        BackupError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        BackupError::MissingKey => (StatusCode::BAD_REQUEST, "missing_key"),
        BackupError::Invalid(_) => (StatusCode::BAD_REQUEST, "invalid_backup"),
        BackupError::SchemaTooNew { .. } => (StatusCode::CONFLICT, "schema_too_new"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "backup_error"),
    };
    (status, Json(json!({ "error": code, "message": e.to_string() }))).into_response()
}

/// Final `include_files`: requested value, but forced on when local-filesystem
/// storage is in use (no redundant copy elsewhere).
fn resolve_include_files(registry: &StorageRegistry, requested: Option<bool>) -> bool {
    let requested = requested.unwrap_or(true);
    if requested {
        return true;
    }
    // Off only allowed when an S3 provider exists (files are redundant there).
    registry.get("s3").is_some()
}

// ── Core operations (shared by instance + site wrappers) ──

async fn create_backup(
    backup: &BackupService,
    registry: &StorageRegistry,
    scope: Scope,
    body: CreateBackupBody,
    created_by: Option<String>,
) -> Response {
    let opts = CreateBackupOptions {
        include_files: resolve_include_files(registry, body.include_files),
        encrypt: body.encrypt,
        scope,
        schedule_id: None,
        created_by,
    };
    match backup.create_backup(opts).await {
        Ok(row) => (StatusCode::CREATED, Json(meta::BackupInfo::from(row))).into_response(),
        Err(e) => err_response(e),
    }
}

async fn list_backups(backup: &BackupService, scope: &str, site_id: Option<&str>) -> Response {
    match meta::list_backups(backup.pool(), Some(scope), site_id).await {
        Ok(rows) => {
            let infos: Vec<meta::BackupInfo> = rows.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(infos)).into_response()
        }
        Err(e) => err_response(e),
    }
}

/// Load a backup row and ensure it belongs to the expected scope/site.
async fn load_scoped_backup(
    backup: &BackupService,
    id: &str,
    expect_site: Option<&str>,
) -> Result<meta::BackupRow, Response> {
    let row = match meta::get_backup(backup.pool(), id).await {
        Ok(Some(r)) => r,
        Ok(None) => return Err(err_response(BackupError::NotFound)),
        Err(e) => return Err(err_response(e)),
    };
    if let Some(site) = expect_site {
        if row.scope != "site" || row.site_id.as_deref() != Some(site) {
            return Err(err_response(BackupError::NotFound));
        }
    }
    Ok(row)
}

async fn delete_backup(backup: &BackupService, id: &str, expect_site: Option<&str>) -> Response {
    if let Err(resp) = load_scoped_backup(backup, id, expect_site).await {
        return resp;
    }
    match backup.delete_backup(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

async fn download_backup(backup: &BackupService, id: &str, expect_site: Option<&str>) -> Response {
    let row = match load_scoped_backup(backup, id, expect_site).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let Some(key) = row.destination_key else {
        return err_response(BackupError::NotFound);
    };
    let bytes = match backup.read_destination(&key).await {
        Ok(b) => b,
        Err(e) => return err_response(e),
    };
    let filename = format!("{id}.cmsbak");
    let mut resp = Response::new(Body::from(bytes));
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
    if let Ok(v) = header::HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
    }
    resp
}

async fn run_restore(backup: &BackupService, source: RestoreSource, target: RestoreTarget, created_by: Option<String>) -> Response {
    match backup.restore(RestoreRequest { source, target, created_by }).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

fn bad_request(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": "bad_request", "message": msg }))).into_response()
}

// ── Schedule core ──

async fn create_schedule(backup: &BackupService, scope: Scope, body: ScheduleBody, created_by: Option<String>) -> Response {
    if let Err(e) = schedule::validate_cron(&body.cron) {
        return err_response(e);
    }
    let next = match schedule::next_run_iso(&body.cron) {
        Ok(n) => n,
        Err(e) => return err_response(e),
    };
    let id = uuid::Uuid::now_v7().to_string();
    let now = crate::services::backup::now_iso();
    let res = meta::create_schedule(
        backup.pool(),
        &id,
        scope.as_str(),
        scope.site_id(),
        &body.cron,
        body.retention_n.unwrap_or(7).max(1),
        body.include_files.unwrap_or(true),
        body.encrypt.unwrap_or(false),
        body.enabled.unwrap_or(true),
        Some(&next),
        created_by.as_deref(),
        &now,
    )
    .await;
    match res {
        Ok(()) => match meta::get_schedule(backup.pool(), &id).await {
            Ok(Some(row)) => (StatusCode::CREATED, Json(meta::ScheduleInfo::from(row))).into_response(),
            _ => err_response(BackupError::NotFound),
        },
        Err(e) => err_response(e),
    }
}

async fn list_schedules(backup: &BackupService, scope: &str, site_id: Option<&str>) -> Response {
    match meta::list_schedules(backup.pool(), Some(scope), site_id).await {
        Ok(rows) => {
            let infos: Vec<meta::ScheduleInfo> = rows.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(infos)).into_response()
        }
        Err(e) => err_response(e),
    }
}

async fn update_schedule(backup: &BackupService, id: &str, expect_site: Option<&str>, body: ScheduleBody) -> Response {
    match meta::get_schedule(backup.pool(), id).await {
        Ok(Some(row)) => {
            if let Some(site) = expect_site {
                if row.scope != "site" || row.site_id.as_deref() != Some(site) {
                    return err_response(BackupError::NotFound);
                }
            }
        }
        Ok(None) => return err_response(BackupError::NotFound),
        Err(e) => return err_response(e),
    }
    if let Err(e) = schedule::validate_cron(&body.cron) {
        return err_response(e);
    }
    let next = schedule::next_run_iso(&body.cron).ok();
    let now = crate::services::backup::now_iso();
    let res = meta::update_schedule(
        backup.pool(),
        id,
        &body.cron,
        body.retention_n.unwrap_or(7).max(1),
        body.include_files.unwrap_or(true),
        body.encrypt.unwrap_or(false),
        body.enabled.unwrap_or(true),
        next.as_deref(),
        &now,
    )
    .await;
    match res {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

async fn delete_schedule(backup: &BackupService, id: &str, expect_site: Option<&str>) -> Response {
    match meta::get_schedule(backup.pool(), id).await {
        Ok(Some(row)) => {
            if let Some(site) = expect_site {
                if row.scope != "site" || row.site_id.as_deref() != Some(site) {
                    return err_response(BackupError::NotFound);
                }
            }
        }
        Ok(None) => return err_response(BackupError::NotFound),
        Err(e) => return err_response(e),
    }
    match meta::delete_schedule(backup.pool(), id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

async fn run_schedule_now(backup: &BackupService, id: &str, expect_site: Option<&str>, created_by: Option<String>) -> Response {
    let row = match meta::get_schedule(backup.pool(), id).await {
        Ok(Some(r)) => r,
        Ok(None) => return err_response(BackupError::NotFound),
        Err(e) => return err_response(e),
    };
    if let Some(site) = expect_site {
        if row.scope != "site" || row.site_id.as_deref() != Some(site) {
            return err_response(BackupError::NotFound);
        }
    }
    let scope = match row.scope.as_str() {
        "site" => match row.site_id.clone() {
            Some(s) => Scope::Site(s),
            None => return err_response(BackupError::Invalid("schedule missing site".into())),
        },
        _ => Scope::Instance,
    };
    let opts = CreateBackupOptions {
        scope,
        include_files: row.include_files != 0,
        encrypt: row.encrypt != 0,
        schedule_id: Some(row.id.clone()),
        created_by,
    };
    match backup.create_backup(opts).await {
        Ok(b) => (StatusCode::CREATED, Json(meta::BackupInfo::from(b))).into_response(),
        Err(e) => err_response(e),
    }
}

/// Resolve a restore source from a JSON body (by backup id or destination key).
async fn resolve_source(backup: &BackupService, body: &RestoreBody) -> Result<RestoreSource, Response> {
    if let Some(id) = &body.backup_id {
        let row = match meta::get_backup(backup.pool(), id).await {
            Ok(Some(r)) => r,
            Ok(None) => return Err(err_response(BackupError::NotFound)),
            Err(e) => return Err(err_response(e)),
        };
        let key = row.destination_key.ok_or_else(|| err_response(BackupError::NotFound))?;
        Ok(RestoreSource::Destination(key))
    } else if let Some(key) = &body.destination_key {
        Ok(RestoreSource::Destination(key.clone()))
    } else {
        Err(bad_request("provide backup_id or destination_key"))
    }
}

// ── Multipart restore parsing ──

struct UploadedRestore {
    bytes: Vec<u8>,
    mode: Option<String>,
    site_id: Option<String>,
    import_as_new: bool,
    confirm: Option<String>,
}

async fn parse_restore_upload(mut multipart: Multipart) -> Result<UploadedRestore, Response> {
    let mut bytes: Option<Vec<u8>> = None;
    let mut mode = None;
    let mut site_id = None;
    let mut import_as_new = false;
    let mut confirm = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name().unwrap_or("") {
            "file" => bytes = field.bytes().await.ok().map(|b| b.to_vec()),
            "mode" => mode = field.text().await.ok(),
            "site_id" => site_id = field.text().await.ok(),
            "import_as_new" => import_as_new = field.text().await.ok().as_deref() == Some("true"),
            "confirm" => confirm = field.text().await.ok(),
            _ => {}
        }
    }
    let Some(bytes) = bytes else {
        return Err(bad_request("no backup file provided"));
    };
    Ok(UploadedRestore { bytes, mode, site_id, import_as_new, confirm })
}

// ───────────────────────────── Instance handlers ─────────────────────────────

async fn require_instance(auth: &AuthContext, repo: &Repository, action: Action) -> Result<String, Response> {
    require_instance_action(auth, repo, action)
        .await
        .map_err(|(s, e)| (s, e).into_response())
}

pub async fn create_instance_backup(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
    Json(body): Json<CreateBackupBody>,
) -> Response {
    let user = match require_instance(&auth, &repository, Action::InstanceBackup).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    create_backup(&backup, &registry, Scope::Instance, body, Some(user)).await
}

pub async fn list_instance_backups(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_instance(&auth, &repository, Action::InstanceBackup).await {
        return r;
    }
    list_backups(&backup, "instance", None).await
}

pub async fn delete_instance_backup(
    auth: AuthContext,
    Path(backup_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_instance(&auth, &repository, Action::InstanceBackup).await {
        return r;
    }
    delete_backup(&backup, &backup_id, None).await
}

pub async fn download_instance_backup(
    auth: AuthContext,
    Path(backup_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_instance(&auth, &repository, Action::InstanceBackup).await {
        return r;
    }
    download_backup(&backup, &backup_id, None).await
}

pub async fn restore_instance(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Json(body): Json<RestoreBody>,
) -> Response {
    let user = match require_instance(&auth, &repository, Action::InstanceRestore).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if body.confirm.as_deref() != Some(RESTORE_CONFIRMATION) {
        return bad_request("restore requires confirm = \"RESTORE\"");
    }
    let source = match resolve_source(&backup, &body).await {
        Ok(s) => s,
        Err(r) => return r,
    };
    let target = match body.mode.as_deref() {
        Some("site") => match body.site_id {
            Some(sid) => RestoreTarget::Site { site_id: sid, import_as_new: body.import_as_new },
            None => return bad_request("site restore requires site_id"),
        },
        _ => RestoreTarget::WholeInstance,
    };
    run_restore(&backup, source, target, Some(user)).await
}

pub async fn restore_instance_upload(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    multipart: Multipart,
) -> Response {
    let user = match require_instance(&auth, &repository, Action::InstanceRestore).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let upload = match parse_restore_upload(multipart).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if upload.confirm.as_deref() != Some(RESTORE_CONFIRMATION) {
        return bad_request("restore requires confirm = \"RESTORE\"");
    }
    let target = match upload.mode.as_deref() {
        Some("site") => match upload.site_id {
            Some(sid) => RestoreTarget::Site { site_id: sid, import_as_new: upload.import_as_new },
            None => return bad_request("site restore requires site_id"),
        },
        _ => RestoreTarget::WholeInstance,
    };
    run_restore(&backup, RestoreSource::Bytes(upload.bytes), target, Some(user)).await
}

pub async fn list_instance_schedules(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_instance(&auth, &repository, Action::InstanceBackup).await {
        return r;
    }
    list_schedules(&backup, "instance", None).await
}

pub async fn create_instance_schedule(
    auth: AuthContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Json(body): Json<ScheduleBody>,
) -> Response {
    let user = match require_instance(&auth, &repository, Action::InstanceBackup).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    create_schedule(&backup, Scope::Instance, body, Some(user)).await
}

pub async fn update_instance_schedule(
    auth: AuthContext,
    Path(schedule_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Json(body): Json<ScheduleBody>,
) -> Response {
    if let Err(r) = require_instance(&auth, &repository, Action::InstanceBackup).await {
        return r;
    }
    update_schedule(&backup, &schedule_id, None, body).await
}

pub async fn delete_instance_schedule(
    auth: AuthContext,
    Path(schedule_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_instance(&auth, &repository, Action::InstanceBackup).await {
        return r;
    }
    delete_schedule(&backup, &schedule_id, None).await
}

pub async fn run_instance_schedule(
    auth: AuthContext,
    Path(schedule_id): Path<String>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    let user = match require_instance(&auth, &repository, Action::InstanceBackup).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    run_schedule_now(&backup, &schedule_id, None, Some(user)).await
}

// ───────────────────────────── Site handlers ─────────────────────────────

async fn require_site(ctx: &RequestContext, repo: &Repository, action: Action) -> Result<String, Response> {
    require_user_action(ctx, repo, action)
        .await
        .map_err(|(s, e)| (s, e).into_response())
}

pub async fn create_site_backup(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Extension(registry): Extension<Arc<StorageRegistry>>,
    Json(body): Json<CreateBackupBody>,
) -> Response {
    let user = match require_site(&ctx, &repository, Action::SiteBackup).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    create_backup(&backup, &registry, Scope::Site(ctx.site_id.clone()), body, Some(user)).await
}

pub async fn list_site_backups(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_site(&ctx, &repository, Action::SiteBackup).await {
        return r;
    }
    list_backups(&backup, "site", Some(&ctx.site_id)).await
}

pub async fn delete_site_backup(
    ctx: RequestContext,
    Path(path): Path<BackupPath>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_site(&ctx, &repository, Action::SiteBackup).await {
        return r;
    }
    delete_backup(&backup, &path.backup_id, Some(&ctx.site_id)).await
}

pub async fn download_site_backup(
    ctx: RequestContext,
    Path(path): Path<BackupPath>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_site(&ctx, &repository, Action::SiteBackup).await {
        return r;
    }
    download_backup(&backup, &path.backup_id, Some(&ctx.site_id)).await
}

pub async fn restore_site(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Json(body): Json<RestoreBody>,
) -> Response {
    let user = match require_site(&ctx, &repository, Action::SiteRestore).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if body.confirm.as_deref() != Some(RESTORE_CONFIRMATION) {
        return bad_request("restore requires confirm = \"RESTORE\"");
    }
    let source = match resolve_source(&backup, &body).await {
        Ok(s) => s,
        Err(r) => return r,
    };
    let target = RestoreTarget::Site {
        site_id: ctx.site_id.clone(),
        import_as_new: body.import_as_new,
    };
    run_restore(&backup, source, target, Some(user)).await
}

pub async fn restore_site_upload(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    multipart: Multipart,
) -> Response {
    let user = match require_site(&ctx, &repository, Action::SiteRestore).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let upload = match parse_restore_upload(multipart).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if upload.confirm.as_deref() != Some(RESTORE_CONFIRMATION) {
        return bad_request("restore requires confirm = \"RESTORE\"");
    }
    let target = RestoreTarget::Site {
        site_id: ctx.site_id.clone(),
        import_as_new: upload.import_as_new,
    };
    run_restore(&backup, RestoreSource::Bytes(upload.bytes), target, Some(user)).await
}

pub async fn list_site_schedules(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_site(&ctx, &repository, Action::SiteBackup).await {
        return r;
    }
    list_schedules(&backup, "site", Some(&ctx.site_id)).await
}

pub async fn create_site_schedule(
    ctx: RequestContext,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Json(body): Json<ScheduleBody>,
) -> Response {
    let user = match require_site(&ctx, &repository, Action::SiteBackup).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    create_schedule(&backup, Scope::Site(ctx.site_id.clone()), body, Some(user)).await
}

pub async fn update_site_schedule(
    ctx: RequestContext,
    Path(path): Path<SchedulePath>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
    Json(body): Json<ScheduleBody>,
) -> Response {
    if let Err(r) = require_site(&ctx, &repository, Action::SiteBackup).await {
        return r;
    }
    update_schedule(&backup, &path.schedule_id, Some(&ctx.site_id), body).await
}

pub async fn delete_site_schedule(
    ctx: RequestContext,
    Path(path): Path<SchedulePath>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    if let Err(r) = require_site(&ctx, &repository, Action::SiteBackup).await {
        return r;
    }
    delete_schedule(&backup, &path.schedule_id, Some(&ctx.site_id)).await
}

pub async fn run_site_schedule(
    ctx: RequestContext,
    Path(path): Path<SchedulePath>,
    Extension(repository): Extension<Repository>,
    Extension(backup): Extension<Arc<BackupService>>,
) -> Response {
    let user = match require_site(&ctx, &repository, Action::SiteBackup).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    run_schedule_now(&backup, &path.schedule_id, Some(&ctx.site_id), Some(user)).await
}
