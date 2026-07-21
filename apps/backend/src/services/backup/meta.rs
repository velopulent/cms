//! CRUD for the backup bookkeeping tables (`backups`, `backup_schedules`,
//! `restore_jobs`). Integer/bool columns are BIGINT (0/1) so a single i64 binds
//! across SQLite/Postgres/MySQL; `?` placeholders are rewritten to `$n` for
//! Postgres by [`q`].

use serde::Serialize;
use sqlx::FromRow;

use super::BackupError;
use crate::database::backend::DatabaseBackend;
use crate::database::pool::DbPool;

fn dberr(e: sqlx::Error) -> BackupError {
    BackupError::Db(e.to_string())
}

/// Rewrite `?` placeholders to `$1..$n` for Postgres; leave as-is otherwise.
fn q(backend: DatabaseBackend, sql: &str) -> String {
    if backend != DatabaseBackend::Postgres {
        return sql.to_string();
    }
    let mut out = String::with_capacity(sql.len() + 8);
    let mut n = 1;
    for ch in sql.chars() {
        if ch == '?' {
            out.push('$');
            out.push_str(&n.to_string());
            n += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

macro_rules! exec {
    ($pool:expr, $sql:expr $(, $bind:expr)* $(,)?) => {{
        let sql = q($pool.backend(), $sql);
        match $pool {
            DbPool::Sqlite(p) => { sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.execute(p).await.map_err(dberr)?; }
            DbPool::Postgres(p) => { sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.execute(p).await.map_err(dberr)?; }
            DbPool::MySql(p) => { sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.execute(p).await.map_err(dberr)?; }
        }
    }};
}

macro_rules! fetch_all_as {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        let sql = q($pool.backend(), $sql);
        match $pool {
            DbPool::Sqlite(p) => sqlx::query_as::<_, $ty>(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.fetch_all(p).await.map_err(dberr)?,
            DbPool::Postgres(p) => sqlx::query_as::<_, $ty>(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.fetch_all(p).await.map_err(dberr)?,
            DbPool::MySql(p) => sqlx::query_as::<_, $ty>(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.fetch_all(p).await.map_err(dberr)?,
        }
    }};
}

macro_rules! fetch_opt_as {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        let sql = q($pool.backend(), $sql);
        match $pool {
            DbPool::Sqlite(p) => sqlx::query_as::<_, $ty>(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.fetch_optional(p).await.map_err(dberr)?,
            DbPool::Postgres(p) => sqlx::query_as::<_, $ty>(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.fetch_optional(p).await.map_err(dberr)?,
            DbPool::MySql(p) => sqlx::query_as::<_, $ty>(sqlx::AssertSqlSafe(sql.as_str()))$(.bind($bind))*.fetch_optional(p).await.map_err(dberr)?,
        }
    }};
}

// ---------------------------------------------------------------------------
// backups
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct BackupRow {
    pub id: String,
    pub schedule_id: Option<String>,
    pub scope: String,
    pub site_id: Option<String>,
    pub status: String,
    pub format_version: i64,
    pub schema_version: Option<String>,
    pub size_bytes: i64,
    pub file_count: i64,
    pub includes_files: i64,
    pub encrypted: i64,
    pub destination_key: Option<String>,
    pub checksum: Option<String>,
    pub error: Option<String>,
    pub created_by: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub storage_profile_id: Option<String>,
}

/// Frontend-friendly view with proper booleans.
#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub id: String,
    pub schedule_id: Option<String>,
    pub scope: String,
    pub site_id: Option<String>,
    pub status: String,
    pub schema_version: Option<String>,
    pub size_bytes: i64,
    pub file_count: i64,
    pub includes_files: bool,
    pub encrypted: bool,
    pub checksum: Option<String>,
    pub error: Option<String>,
    pub created_by: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub storage_profile_id: Option<String>,
}

impl From<BackupRow> for BackupInfo {
    fn from(r: BackupRow) -> Self {
        BackupInfo {
            id: r.id,
            schedule_id: r.schedule_id,
            scope: r.scope,
            site_id: r.site_id,
            status: r.status,
            schema_version: r.schema_version,
            size_bytes: r.size_bytes,
            file_count: r.file_count,
            includes_files: r.includes_files != 0,
            encrypted: r.encrypted != 0,
            checksum: r.checksum,
            error: r.error,
            created_by: r.created_by,
            completed_at: r.completed_at,
            created_at: r.created_at,
            storage_profile_id: r.storage_profile_id,
        }
    }
}

const BACKUP_COLS: &str = "id, schedule_id, scope, site_id, status, format_version, schema_version, \
    size_bytes, file_count, includes_files, encrypted, destination_key, checksum, error, created_by, \
    started_at, completed_at, created_at, storage_profile_id";

/// Insert a `running` backup row at the start of a run.
#[allow(clippy::too_many_arguments)]
pub async fn insert_running(
    pool: &DbPool,
    id: &str,
    schedule_id: Option<&str>,
    scope: &str,
    site_id: Option<&str>,
    includes_files: bool,
    encrypt: bool,
    created_by: Option<&str>,
    storage_profile_id: Option<&str>,
    now: &str,
) -> Result<(), BackupError> {
    exec!(
        pool,
        "INSERT INTO backups (id, schedule_id, scope, site_id, status, format_version, includes_files, encrypted, created_by, started_at, created_at, storage_profile_id) \
         VALUES (?, ?, ?, ?, 'running', ?, ?, ?, ?, ?, ?, ?)",
        id.to_string(),
        schedule_id.map(|s| s.to_string()),
        scope.to_string(),
        site_id.map(|s| s.to_string()),
        super::FORMAT_VERSION,
        i64::from(includes_files),
        i64::from(encrypt),
        created_by.map(|s| s.to_string()),
        now.to_string(),
        storage_profile_id.map(|value| value.to_string()),
        now.to_string(),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn mark_success(
    pool: &DbPool,
    id: &str,
    schema_version: i64,
    size_bytes: i64,
    file_count: i64,
    destination_key: &str,
    checksum: &str,
    now: &str,
) -> Result<(), BackupError> {
    exec!(
        pool,
        "UPDATE backups SET status = 'success', schema_version = ?, size_bytes = ?, file_count = ?, \
         destination_key = ?, checksum = ?, completed_at = ? WHERE id = ?",
        schema_version.to_string(),
        size_bytes,
        file_count,
        destination_key.to_string(),
        checksum.to_string(),
        now.to_string(),
        id.to_string(),
    );
    Ok(())
}

pub async fn mark_failed(pool: &DbPool, id: &str, error: &str, now: &str) -> Result<(), BackupError> {
    exec!(
        pool,
        "UPDATE backups SET status = 'failed', error = ?, completed_at = ? WHERE id = ?",
        error.to_string(),
        now.to_string(),
        id.to_string(),
    );
    Ok(())
}

/// Fail backups/restore jobs left mid-flight by a previous process. Any
/// `running`/`pending` row at startup is orphaned, since backups only ever run
/// in-process. Returns the number of backup rows reconciled (for logging).
pub async fn fail_orphaned(pool: &DbPool, now: &str) -> Result<u64, BackupError> {
    let backups_sql = q(
        pool.backend(),
        "UPDATE backups SET status = 'failed', error = 'interrupted: server stopped during backup', \
         completed_at = ? WHERE status IN ('running', 'pending')",
    );
    let reconciled = match pool {
        DbPool::Sqlite(p) => sqlx::query(sqlx::AssertSqlSafe(backups_sql.as_str()))
            .bind(now.to_string())
            .execute(p)
            .await
            .map_err(dberr)?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query(sqlx::AssertSqlSafe(backups_sql.as_str()))
            .bind(now.to_string())
            .execute(p)
            .await
            .map_err(dberr)?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query(sqlx::AssertSqlSafe(backups_sql.as_str()))
            .bind(now.to_string())
            .execute(p)
            .await
            .map_err(dberr)?
            .rows_affected(),
    };

    exec!(
        pool,
        "UPDATE restore_jobs SET status = 'failed', error = 'interrupted: server stopped during restore', \
         completed_at = ? WHERE status IN ('running', 'pending')",
        now.to_string(),
    );

    Ok(reconciled)
}

pub async fn get_backup(pool: &DbPool, id: &str) -> Result<Option<BackupRow>, BackupError> {
    let sql = format!("SELECT {BACKUP_COLS} FROM backups WHERE id = ?");
    Ok(fetch_opt_as!(pool, BackupRow, &sql, id.to_string()))
}

/// List backups, optionally filtered to a scope/site.
pub async fn list_backups(
    pool: &DbPool,
    scope: Option<&str>,
    site_id: Option<&str>,
) -> Result<Vec<BackupRow>, BackupError> {
    let rows = match (scope, site_id) {
        (Some(sc), Some(sid)) => {
            let sql =
                format!("SELECT {BACKUP_COLS} FROM backups WHERE scope = ? AND site_id = ? ORDER BY created_at DESC");
            fetch_all_as!(pool, BackupRow, &sql, sc.to_string(), sid.to_string())
        }
        (Some(sc), None) => {
            let sql = format!("SELECT {BACKUP_COLS} FROM backups WHERE scope = ? ORDER BY created_at DESC");
            fetch_all_as!(pool, BackupRow, &sql, sc.to_string())
        }
        _ => {
            let sql = format!("SELECT {BACKUP_COLS} FROM backups ORDER BY created_at DESC");
            fetch_all_as!(pool, BackupRow, &sql)
        }
    };
    Ok(rows)
}

pub async fn delete_backup_row(pool: &DbPool, id: &str) -> Result<(), BackupError> {
    exec!(pool, "DELETE FROM backups WHERE id = ?", id.to_string());
    Ok(())
}

/// Successful backups for a schedule, oldest first (for retention pruning).
pub async fn schedule_successful_backups(pool: &DbPool, schedule_id: &str) -> Result<Vec<BackupRow>, BackupError> {
    let sql = format!(
        "SELECT {BACKUP_COLS} FROM backups WHERE schedule_id = ? AND status = 'success' ORDER BY created_at ASC"
    );
    Ok(fetch_all_as!(pool, BackupRow, &sql, schedule_id.to_string()))
}

// ---------------------------------------------------------------------------
// backup_schedules
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct BackupScheduleRow {
    pub id: String,
    pub scope: String,
    pub site_id: Option<String>,
    pub cron: String,
    pub retention_n: i64,
    pub include_files: i64,
    pub encrypt: i64,
    pub enabled: i64,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub storage_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleInfo {
    pub id: String,
    pub scope: String,
    pub site_id: Option<String>,
    pub cron: String,
    pub retention_n: i64,
    pub include_files: bool,
    pub encrypt: bool,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub storage_profile_id: Option<String>,
}

impl From<BackupScheduleRow> for ScheduleInfo {
    fn from(r: BackupScheduleRow) -> Self {
        ScheduleInfo {
            id: r.id,
            scope: r.scope,
            site_id: r.site_id,
            cron: r.cron,
            retention_n: r.retention_n,
            include_files: r.include_files != 0,
            encrypt: r.encrypt != 0,
            enabled: r.enabled != 0,
            last_run_at: r.last_run_at,
            next_run_at: r.next_run_at,
            created_at: r.created_at,
            storage_profile_id: r.storage_profile_id,
        }
    }
}

const SCHEDULE_COLS: &str = "id, scope, site_id, cron, retention_n, include_files, encrypt, enabled, \
    last_run_at, next_run_at, created_by, created_at, updated_at, storage_profile_id";

#[allow(clippy::too_many_arguments)]
pub async fn create_schedule(
    pool: &DbPool,
    id: &str,
    scope: &str,
    site_id: Option<&str>,
    cron: &str,
    retention_n: i64,
    include_files: bool,
    encrypt: bool,
    enabled: bool,
    next_run_at: Option<&str>,
    created_by: Option<&str>,
    storage_profile_id: Option<&str>,
    now: &str,
) -> Result<(), BackupError> {
    exec!(
        pool,
        "INSERT INTO backup_schedules (id, scope, site_id, cron, retention_n, include_files, encrypt, enabled, next_run_at, created_by, created_at, updated_at, storage_profile_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id.to_string(),
        scope.to_string(),
        site_id.map(|s| s.to_string()),
        cron.to_string(),
        retention_n,
        i64::from(include_files),
        i64::from(encrypt),
        i64::from(enabled),
        next_run_at.map(|s| s.to_string()),
        created_by.map(|s| s.to_string()),
        now.to_string(),
        storage_profile_id.map(|value| value.to_string()),
        now.to_string(),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_schedule(
    pool: &DbPool,
    id: &str,
    cron: &str,
    retention_n: i64,
    include_files: bool,
    encrypt: bool,
    enabled: bool,
    next_run_at: Option<&str>,
    storage_profile_id: Option<&str>,
    now: &str,
) -> Result<(), BackupError> {
    exec!(
        pool,
        "UPDATE backup_schedules SET cron = ?, retention_n = ?, include_files = ?, encrypt = ?, enabled = ?, next_run_at = ?, storage_profile_id = ?, updated_at = ? WHERE id = ?",
        cron.to_string(),
        retention_n,
        i64::from(include_files),
        i64::from(encrypt),
        i64::from(enabled),
        next_run_at.map(|s| s.to_string()),
        storage_profile_id.map(|value| value.to_string()),
        now.to_string(),
        id.to_string(),
    );
    Ok(())
}

pub async fn set_schedule_runs(
    pool: &DbPool,
    id: &str,
    last_run_at: &str,
    next_run_at: Option<&str>,
) -> Result<(), BackupError> {
    exec!(
        pool,
        "UPDATE backup_schedules SET last_run_at = ?, next_run_at = ? WHERE id = ?",
        last_run_at.to_string(),
        next_run_at.map(|s| s.to_string()),
        id.to_string(),
    );
    Ok(())
}

pub async fn delete_schedule(pool: &DbPool, id: &str) -> Result<(), BackupError> {
    exec!(pool, "DELETE FROM backup_schedules WHERE id = ?", id.to_string());
    Ok(())
}

pub async fn get_schedule(pool: &DbPool, id: &str) -> Result<Option<BackupScheduleRow>, BackupError> {
    let sql = format!("SELECT {SCHEDULE_COLS} FROM backup_schedules WHERE id = ?");
    Ok(fetch_opt_as!(pool, BackupScheduleRow, &sql, id.to_string()))
}

pub async fn list_schedules(
    pool: &DbPool,
    scope: Option<&str>,
    site_id: Option<&str>,
) -> Result<Vec<BackupScheduleRow>, BackupError> {
    let rows = match (scope, site_id) {
        (Some(sc), Some(sid)) => {
            let sql = format!(
                "SELECT {SCHEDULE_COLS} FROM backup_schedules WHERE scope = ? AND site_id = ? ORDER BY created_at DESC"
            );
            fetch_all_as!(pool, BackupScheduleRow, &sql, sc.to_string(), sid.to_string())
        }
        (Some(sc), None) => {
            let sql = format!("SELECT {SCHEDULE_COLS} FROM backup_schedules WHERE scope = ? ORDER BY created_at DESC");
            fetch_all_as!(pool, BackupScheduleRow, &sql, sc.to_string())
        }
        _ => {
            let sql = format!("SELECT {SCHEDULE_COLS} FROM backup_schedules ORDER BY created_at DESC");
            fetch_all_as!(pool, BackupScheduleRow, &sql)
        }
    };
    Ok(rows)
}

/// Enabled schedules whose `next_run_at` is due (<= now) or unset.
pub async fn due_schedules(pool: &DbPool, now: &str) -> Result<Vec<BackupScheduleRow>, BackupError> {
    let sql = format!(
        "SELECT {SCHEDULE_COLS} FROM backup_schedules WHERE enabled = 1 AND (next_run_at IS NULL OR next_run_at <= ?) ORDER BY created_at ASC"
    );
    Ok(fetch_all_as!(pool, BackupScheduleRow, &sql, now.to_string()))
}

// ---------------------------------------------------------------------------
// restore_jobs (audit)
// ---------------------------------------------------------------------------

pub async fn insert_restore_running(
    pool: &DbPool,
    id: &str,
    source: &str,
    scope: &str,
    target_site_id: Option<&str>,
    created_by: Option<&str>,
    now: &str,
) -> Result<(), BackupError> {
    exec!(
        pool,
        "INSERT INTO restore_jobs (id, source, scope, target_site_id, status, created_by, started_at, created_at) \
         VALUES (?, ?, ?, ?, 'running', ?, ?, ?)",
        id.to_string(),
        source.to_string(),
        scope.to_string(),
        target_site_id.map(|s| s.to_string()),
        created_by.map(|s| s.to_string()),
        now.to_string(),
        now.to_string(),
    );
    Ok(())
}

pub async fn mark_restore_done(
    pool: &DbPool,
    id: &str,
    status: &str,
    error: Option<&str>,
    now: &str,
) -> Result<(), BackupError> {
    exec!(
        pool,
        "UPDATE restore_jobs SET status = ?, error = ?, completed_at = ? WHERE id = ?",
        status.to_string(),
        error.map(|s| s.to_string()),
        now.to_string(),
        id.to_string(),
    );
    Ok(())
}
