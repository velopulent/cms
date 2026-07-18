//! Table registry and cross-backend logical dump/restore.
//!
//! Every dumped value is normalized to **text** on read (Postgres casts with
//! `::text`, booleans via `::int`), so a backup is a portable, DB-agnostic set of
//! NDJSON rows. On restore the values are bound back as strings; only Postgres
//! needs per-column casts (`::jsonb`, `::timestamptz`, `::bigint`,
//! `::int::boolean`) because it is strictly typed — SQLite and MySQL coerce.

use sqlx::Row;

use super::{BackupError, Scope};
use crate::database::backend::DatabaseBackend;
use crate::database::pool::DbPool;

/// Logical column type — drives the text-cast on dump and the cast on Postgres insert.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColType {
    Text,
    Int,
    Bool,
    Json,
    Timestamp,
}

pub struct Column {
    pub name: &'static str,
    pub ty: ColType,
}

const fn col(name: &'static str, ty: ColType) -> Column {
    Column { name, ty }
}

/// How a table is scoped to a single site (for site-scope backups).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SiteWhere {
    /// Table has a `site_id` column.
    SiteId,
    /// The `sites` table itself (filter by `id`).
    SitesId,
    /// Instance-only table (e.g. `users`) — excluded from a site backup.
    InstanceOnly,
    /// `entry_revisions`: scoped via the parent entry's site.
    EntryRevisions,
    /// `site_webhook_deliveries`: scoped via the parent webhook's site.
    WebhookDeliveries,
}

pub struct TableSpec {
    pub name: &'static str,
    pub columns: &'static [Column],
    pub site_where: SiteWhere,
}

use ColType::{Bool, Int, Json, Text, Timestamp};

/// All dumped tables, in FK parent→child order (the order restore inserts in).
pub static TABLES: &[TableSpec] = &[
    TableSpec {
        name: "users",
        site_where: SiteWhere::InstanceOnly,
        columns: &[
            col("id", Text),
            col("name", Text),
            col("email", Text),
            col("password_hash", Text),
            col("instance_role", Text),
            col("must_change_password", Bool),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
        ],
    },
    TableSpec {
        name: "sites",
        site_where: SiteWhere::SitesId,
        columns: &[
            col("id", Text),
            col("name", Text),
            col("storage_provider", Text),
            col("created_by", Text),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
        ],
    },
    TableSpec {
        name: "site_members",
        site_where: SiteWhere::SiteId,
        columns: &[
            col("id", Text),
            col("site_id", Text),
            col("user_id", Text),
            col("role", Text),
            col("created_at", Timestamp),
        ],
    },
    TableSpec {
        name: "collections",
        site_where: SiteWhere::SiteId,
        columns: &[
            col("id", Text),
            col("site_id", Text),
            col("name", Text),
            col("slug", Text),
            col("definition", Json),
            col("is_singleton", Bool),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
        ],
    },
    TableSpec {
        name: "entries",
        site_where: SiteWhere::SiteId,
        columns: &[
            col("id", Text),
            col("site_id", Text),
            col("collection_id", Text),
            col("data", Json),
            col("slug", Text),
            col("status", Text),
            col("singleton_collection_id", Text),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
            col("published_at", Timestamp),
        ],
    },
    TableSpec {
        name: "files",
        site_where: SiteWhere::SiteId,
        columns: &[
            col("id", Text),
            col("site_id", Text),
            col("filename", Text),
            col("original_name", Text),
            col("mime_type", Text),
            col("size", Int),
            col("storage_provider", Text),
            col("storage_key", Text),
            col("thumbnail_key", Text),
            col("width", Int),
            col("height", Int),
            col("deleted_at", Timestamp),
            col("created_by", Text),
            col("created_at", Timestamp),
        ],
    },
    TableSpec {
        name: "entry_file_references",
        site_where: SiteWhere::SiteId,
        columns: &[col("entry_id", Text), col("file_id", Text), col("site_id", Text)],
    },
    TableSpec {
        name: "entry_revisions",
        site_where: SiteWhere::EntryRevisions,
        columns: &[
            col("id", Text),
            col("entry_id", Text),
            col("revision_number", Int),
            col("data", Json),
            col("created_by", Text),
            col("created_at", Timestamp),
            col("change_summary", Text),
        ],
    },
    TableSpec {
        name: "access_tokens",
        site_where: SiteWhere::SiteId,
        columns: &[
            col("id", Text),
            col("site_id", Text),
            col("name", Text),
            col("token_hash", Text),
            col("token_prefix", Text),
            col("token_hmac", Text),
            col("permission", Text),
            col("created_by_user_id", Text),
            col("last_used_at", Timestamp),
            col("created_at", Timestamp),
            col("expires_at", Timestamp),
            col("revoked_at", Timestamp),
        ],
    },
    TableSpec {
        name: "site_webhooks",
        site_where: SiteWhere::SiteId,
        columns: &[
            col("id", Text),
            col("site_id", Text),
            col("label", Text),
            col("url", Text),
            col("headers_encrypted", Text),
            col("enabled", Bool),
            col("created_by", Text),
            col("created_at", Timestamp),
            col("updated_at", Timestamp),
        ],
    },
    TableSpec {
        name: "site_webhook_deliveries",
        site_where: SiteWhere::WebhookDeliveries,
        columns: &[
            col("id", Text),
            col("webhook_id", Text),
            col("status", Text),
            col("status_code", Int),
            col("response_body", Text),
            col("duration_ms", Int),
            col("triggered_by", Text),
            col("triggered_at", Timestamp),
        ],
    },
];

pub fn table_spec(name: &str) -> Option<&'static TableSpec> {
    TABLES.iter().find(|t| t.name == name)
}

/// A dumped table: column names plus rows of text-normalized values.
pub struct DumpedTable {
    pub name: &'static str,
    pub columns: Vec<&'static str>,
    pub rows: Vec<Vec<Option<String>>>,
}

/// Select expression for a column, casting non-text values to text per backend.
fn select_expr(backend: DatabaseBackend, c: &Column) -> String {
    match backend {
        DatabaseBackend::Postgres => match c.ty {
            ColType::Text => c.name.to_string(),
            ColType::Bool => format!("({}::int)::text AS {}", c.name, c.name),
            _ => format!("{}::text AS {}", c.name, c.name),
        },
        DatabaseBackend::SQLite => match c.ty {
            ColType::Text | ColType::Json | ColType::Timestamp => c.name.to_string(),
            ColType::Int | ColType::Bool => format!("CAST({} AS TEXT) AS {}", c.name, c.name),
        },
        DatabaseBackend::MySQL => match c.ty {
            ColType::Text => c.name.to_string(),
            _ => format!("CAST({} AS CHAR) AS {}", c.name, c.name),
        },
    }
}

fn site_param_placeholder(backend: DatabaseBackend) -> &'static str {
    match backend {
        DatabaseBackend::Postgres => "$1",
        _ => "?",
    }
}

/// Build the SELECT for a table under the given scope. Returns `None` when the
/// table is not part of a site-scope backup.
fn select_sql(backend: DatabaseBackend, spec: &TableSpec, scope: &Scope) -> Option<(String, Option<String>)> {
    let cols = spec
        .columns
        .iter()
        .map(|c| select_expr(backend, c))
        .collect::<Vec<_>>()
        .join(", ");
    let base = format!("SELECT {} FROM {}", cols, spec.name);

    match scope {
        Scope::Instance => Some((base, None)),
        Scope::Site(site_id) => {
            let ph = site_param_placeholder(backend);
            let where_clause = match spec.site_where {
                SiteWhere::InstanceOnly => return None,
                SiteWhere::SiteId => format!(" WHERE site_id = {ph}"),
                SiteWhere::SitesId => format!(" WHERE id = {ph}"),
                SiteWhere::EntryRevisions => {
                    format!(" WHERE entry_id IN (SELECT id FROM entries WHERE site_id = {ph})")
                }
                SiteWhere::WebhookDeliveries => {
                    format!(" WHERE webhook_id IN (SELECT id FROM site_webhooks WHERE site_id = {ph})")
                }
            };
            Some((format!("{base}{where_clause}"), Some(site_id.clone())))
        }
    }
}

/// Read all in-scope rows of every table as text-normalized values.
pub async fn dump_tables(pool: &DbPool, scope: &Scope) -> Result<Vec<DumpedTable>, BackupError> {
    let backend = pool.backend();
    let mut out = Vec::new();
    for spec in TABLES {
        let Some((sql, param)) = select_sql(backend, spec, scope) else {
            continue;
        };
        let mut rows = fetch_rows(pool, &sql, param.as_deref(), spec.columns.len()).await?;
        if spec.name == "site_webhooks" {
            for row in &mut rows {
                row[4] = Some(String::new());
                row[5] = Some("0".into());
            }
        }
        out.push(DumpedTable {
            name: spec.name,
            columns: spec.columns.iter().map(|c| c.name).collect(),
            rows,
        });
    }
    Ok(out)
}

/// Fetch rows where every column is read as `Option<String>` (the SELECT already
/// cast non-text columns to text).
pub async fn fetch_rows(
    pool: &DbPool,
    sql: &str,
    param: Option<&str>,
    ncols: usize,
) -> Result<Vec<Vec<Option<String>>>, BackupError> {
    macro_rules! run {
        ($p:expr) => {{
            let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
            if let Some(v) = param {
                q = q.bind(v.to_string());
            }
            let rows = q.fetch_all($p).await.map_err(|e| BackupError::Db(e.to_string()))?;
            let mut out = Vec::with_capacity(rows.len());
            for r in &rows {
                let mut vals = Vec::with_capacity(ncols);
                for i in 0..ncols {
                    let v: Option<String> = r.try_get(i).map_err(|e| BackupError::Db(e.to_string()))?;
                    vals.push(v);
                }
                out.push(vals);
            }
            out
        }};
    }
    Ok(match pool {
        DbPool::Sqlite(p) => run!(p),
        DbPool::Postgres(p) => run!(p),
        DbPool::MySql(p) => run!(p),
    })
}

/// Build the INSERT statement for a table (placeholders + Postgres casts).
pub fn insert_sql(backend: DatabaseBackend, spec: &TableSpec) -> String {
    let names = spec.columns.iter().map(|c| c.name).collect::<Vec<_>>().join(", ");
    let placeholders = spec
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| match backend {
            DatabaseBackend::Postgres => {
                let n = i + 1;
                match c.ty {
                    ColType::Text => format!("${n}"),
                    ColType::Json => format!("${n}::jsonb"),
                    ColType::Timestamp => format!("${n}::timestamptz"),
                    ColType::Int => format!("${n}::bigint"),
                    ColType::Bool => format!("${n}::int::boolean"),
                }
            }
            _ => "?".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("INSERT INTO {} ({}) VALUES ({})", spec.name, names, placeholders)
}

/// One delete or insert step in a restore plan.
pub struct Statement {
    pub sql: String,
    /// For inserts: one entry per row. For deletes: a single param set (often empty).
    pub rows: Vec<Vec<Option<String>>>,
}

/// A fully-resolved restore plan: ordered deletes then ordered inserts. All SQL
/// is backend-specific text; rows carry the final (possibly reconciled) values.
pub struct RestorePlan {
    pub deletes: Vec<Statement>,
    pub inserts: Vec<Statement>,
}

/// Apply a restore plan atomically in a single transaction.
pub async fn apply_restore(pool: &DbPool, plan: &RestorePlan) -> Result<(), BackupError> {
    macro_rules! exec {
        ($tx:expr) => {{
            for stmt in &plan.deletes {
                if stmt.rows.is_empty() {
                    sqlx::query(sqlx::AssertSqlSafe(stmt.sql.as_str()))
                        .execute(&mut *$tx)
                        .await
                        .map_err(|e| BackupError::Db(e.to_string()))?;
                } else {
                    for row in &stmt.rows {
                        let mut q = sqlx::query(sqlx::AssertSqlSafe(stmt.sql.as_str()));
                        for v in row {
                            q = q.bind(v.clone());
                        }
                        q.execute(&mut *$tx).await.map_err(|e| BackupError::Db(e.to_string()))?;
                    }
                }
            }
            for stmt in &plan.inserts {
                for row in &stmt.rows {
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(stmt.sql.as_str()));
                    for v in row {
                        q = q.bind(v.clone());
                    }
                    q.execute(&mut *$tx).await.map_err(|e| BackupError::Db(e.to_string()))?;
                }
            }
        }};
    }

    match pool {
        DbPool::Sqlite(p) => {
            let mut tx = p.begin().await.map_err(|e| BackupError::Db(e.to_string()))?;
            exec!(tx);
            tx.commit().await.map_err(|e| BackupError::Db(e.to_string()))?;
        }
        DbPool::Postgres(p) => {
            let mut tx = p.begin().await.map_err(|e| BackupError::Db(e.to_string()))?;
            exec!(tx);
            tx.commit().await.map_err(|e| BackupError::Db(e.to_string()))?;
        }
        DbPool::MySql(p) => {
            let mut tx = p.begin().await.map_err(|e| BackupError::Db(e.to_string()))?;
            exec!(tx);
            tx.commit().await.map_err(|e| BackupError::Db(e.to_string()))?;
        }
    }
    Ok(())
}

/// Fetch the set of existing user ids in the target DB (for reconciling
/// site-scope restores whose backups don't carry the `users` table).
pub async fn existing_user_ids(pool: &DbPool) -> Result<std::collections::HashSet<String>, BackupError> {
    let rows = fetch_rows(pool, "SELECT id FROM users", None, 1).await?;
    Ok(rows.into_iter().filter_map(|mut r| r.pop().flatten()).collect())
}

/// Whether a site id already exists in the target DB.
pub async fn site_exists(pool: &DbPool, site_id: &str) -> Result<bool, BackupError> {
    let backend = pool.backend();
    let ph = site_param_placeholder(backend);
    let sql = format!("SELECT id FROM sites WHERE id = {ph}");
    let rows = fetch_rows(pool, &sql, Some(site_id), 1).await?;
    Ok(!rows.is_empty())
}
