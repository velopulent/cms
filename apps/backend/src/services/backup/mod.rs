//! Backup & restore engine.
//!
//! A backup is a portable logical dump: every in-scope table is read as
//! text-normalized NDJSON (see [`schema`]), bundled in a tar alongside a
//! `manifest.json` and (optionally) uploaded file blobs, then zstd-compressed and
//! optionally AES-256-GCM encrypted. Restore reverses this inside a single write
//! transaction (full-replace within scope). See `meta` for bookkeeping tables.

pub mod meta;
pub mod schedule;
pub mod scheduler;
pub mod schema;

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::database::pool::DbPool;
use crate::storage::{FileSystemStorage, S3Storage, StorageProvider, StorageRegistry};

pub use schema::TABLES;

/// On-disk backup format version. Bumped only on breaking artifact-layout changes.
pub const FORMAT_VERSION: i64 = 1;
const MAGIC: &[u8] = b"CMSBKP1";
const FLAG_ENCRYPTED: u8 = 0b0000_0001;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("database error: {0}")]
    Db(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("encryption error: {0}")]
    Crypto(String),
    #[error("invalid backup: {0}")]
    Invalid(String),
    #[error("backup is encrypted but no encryption key is configured")]
    MissingKey,
    #[error("backup schema version {backup} is newer than this instance ({current}); upgrade before restoring")]
    SchemaTooNew { backup: i64, current: i64 },
    #[error("not found")]
    NotFound,
}

/// What a backup covers.
#[derive(Clone, Debug)]
pub enum Scope {
    Instance,
    Site(String),
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Instance => "instance",
            Scope::Site(_) => "site",
        }
    }
    pub fn site_id(&self) -> Option<&str> {
        match self {
            Scope::Instance => None,
            Scope::Site(s) => Some(s),
        }
    }
}

// --- Manifest ---------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
pub struct TableManifest {
    pub name: String,
    pub row_count: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileManifest {
    pub storage_key: String,
    pub provider: String,
    pub size: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub format_version: i64,
    pub schema_version: i64,
    pub scope: String,
    pub site_id: Option<String>,
    pub created_at: String,
    pub includes_files: bool,
    pub encrypted: bool,
    pub compression: String,
    pub tables: Vec<TableManifest>,
    pub files: Vec<FileManifest>,
}

// --- Request/options types --------------------------------------------------

pub struct CreateBackupOptions {
    pub scope: Scope,
    pub include_files: bool,
    pub encrypt: bool,
    pub schedule_id: Option<String>,
    pub created_by: Option<String>,
}

/// Where a restore reads its artifact from.
pub enum RestoreSource {
    Destination(String),
    Bytes(Vec<u8>),
}

/// What part of a backup to restore and how to handle ids.
pub enum RestoreTarget {
    /// Restore the whole instance (instance backups only).
    WholeInstance,
    /// Restore a single site. `site_id` is the site within the backup. When
    /// `import_as_new`, all ids are remapped so the site is added as a copy.
    Site { site_id: String, import_as_new: bool },
}

pub struct RestoreRequest {
    pub source: RestoreSource,
    pub target: RestoreTarget,
    pub created_by: Option<String>,
}

// --- Service ----------------------------------------------------------------

#[derive(Clone)]
pub struct BackupService {
    pool: DbPool,
    storage: Arc<StorageRegistry>,
    destination: Arc<dyn StorageProvider>,
    encryption_key: Option<[u8; 32]>,
    zstd_level: i32,
}

impl BackupService {
    pub fn new(
        pool: DbPool,
        storage: Arc<StorageRegistry>,
        destination: Arc<dyn StorageProvider>,
        config: &Config,
    ) -> Self {
        let encryption_key = config.backup_encryption_key.as_deref().and_then(parse_key_hex);
        Self {
            pool,
            storage,
            destination,
            encryption_key,
            zstd_level: config.backup_zstd_level,
        }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    // --- Backup ---

    /// Build a backup artifact in memory, returning its manifest and bytes.
    pub async fn build_artifact(
        &self,
        scope: &Scope,
        include_files: bool,
        encrypt: bool,
    ) -> Result<(Manifest, Vec<u8>), BackupError> {
        if encrypt && self.encryption_key.is_none() {
            return Err(BackupError::MissingKey);
        }

        let dumped = schema::dump_tables(&self.pool, scope).await?;

        // Serialize tables to NDJSON + collect file blobs from the `files` table.
        let mut tar_tables: Vec<(String, Vec<u8>)> = Vec::new();
        let mut table_manifest: Vec<TableManifest> = Vec::new();
        let mut file_blobs: Vec<(String, Vec<u8>)> = Vec::new();
        let mut file_manifest: Vec<FileManifest> = Vec::new();

        for t in &dumped {
            let ndjson = table_to_ndjson(t);
            table_manifest.push(TableManifest {
                name: t.name.to_string(),
                row_count: t.rows.len(),
            });
            tar_tables.push((format!("tables/{}.ndjson", t.name), ndjson));

            if include_files && t.name == "files" {
                let (blobs, manifest) = self.collect_file_blobs(t).await;
                file_blobs = blobs;
                file_manifest = manifest;
            }
        }

        let manifest = Manifest {
            format_version: FORMAT_VERSION,
            schema_version: crate::database::latest_migration_version(self.pool.backend()),
            scope: scope.as_str().to_string(),
            site_id: scope.site_id().map(String::from),
            created_at: now_iso(),
            includes_files: include_files,
            encrypted: encrypt,
            compression: "zstd".to_string(),
            tables: table_manifest,
            files: file_manifest,
        };

        let tar_bytes = build_tar(&manifest, &tar_tables, &file_blobs)?;
        let compressed =
            zstd::stream::encode_all(&tar_bytes[..], self.zstd_level).map_err(|e| BackupError::Io(e.to_string()))?;
        let outer = self.wrap(compressed, encrypt)?;
        Ok((manifest, outer))
    }

    /// Run a backup: build the artifact, write it to the destination, and record
    /// a row in `backups`. On failure the row is marked `failed`.
    pub async fn create_backup(&self, opts: CreateBackupOptions) -> Result<meta::BackupRow, BackupError> {
        let id = new_id();
        let now = now_iso();
        meta::insert_running(
            &self.pool,
            &id,
            opts.schedule_id.as_deref(),
            opts.scope.as_str(),
            opts.scope.site_id(),
            opts.include_files,
            opts.encrypt,
            opts.created_by.as_deref(),
            &now,
        )
        .await?;

        match self.run_backup(&id, &opts).await {
            Ok((bytes, manifest, key)) => {
                let checksum = sha256_hex(&bytes);
                meta::mark_success(
                    &self.pool,
                    &id,
                    manifest.schema_version,
                    bytes.len() as i64,
                    manifest.files.len() as i64,
                    &key,
                    &checksum,
                    &now_iso(),
                )
                .await?;
            }
            Err(e) => {
                let _ = meta::mark_failed(&self.pool, &id, &e.to_string(), &now_iso()).await;
                return Err(e);
            }
        }

        // Retention pruning for scheduled backups.
        if let Some(sched_id) = &opts.schedule_id {
            if let Ok(Some(sched)) = meta::get_schedule(&self.pool, sched_id).await {
                let _ = self.prune_retention(sched_id, sched.retention_n).await;
            }
        }

        meta::get_backup(&self.pool, &id).await?.ok_or(BackupError::NotFound)
    }

    async fn run_backup(
        &self,
        id: &str,
        opts: &CreateBackupOptions,
    ) -> Result<(Vec<u8>, Manifest, String), BackupError> {
        let (manifest, bytes) = self
            .build_artifact(&opts.scope, opts.include_files, opts.encrypt)
            .await?;
        let key = match &opts.scope {
            Scope::Instance => format!("instance/{id}.cmsbak"),
            Scope::Site(sid) => format!("site/{sid}/{id}.cmsbak"),
        };
        self.destination
            .put(&key, bytes::Bytes::from(bytes.clone()), "application/octet-stream")
            .await
            .map_err(|e| BackupError::Storage(e.to_string()))?;
        Ok((bytes, manifest, key))
    }

    /// Delete a backup artifact and its row.
    pub async fn delete_backup(&self, id: &str) -> Result<(), BackupError> {
        if let Some(row) = meta::get_backup(&self.pool, id).await? {
            if let Some(key) = &row.destination_key {
                let _ = self.destination.delete(key).await;
            }
        }
        meta::delete_backup_row(&self.pool, id).await
    }

    /// Keep only the newest `retention_n` successful backups for a schedule.
    async fn prune_retention(&self, schedule_id: &str, retention_n: i64) -> Result<(), BackupError> {
        if retention_n <= 0 {
            return Ok(());
        }
        let mut rows = meta::schedule_successful_backups(&self.pool, schedule_id).await?; // oldest first
        let excess = rows.len() as i64 - retention_n;
        if excess <= 0 {
            return Ok(());
        }
        rows.truncate(excess as usize);
        for row in rows {
            let _ = self.delete_backup(&row.id).await;
        }
        Ok(())
    }

    async fn collect_file_blobs(&self, files: &schema::DumpedTable) -> (Vec<(String, Vec<u8>)>, Vec<FileManifest>) {
        let idx = |name: &str| files.columns.iter().position(|c| *c == name);
        let (provider_i, key_i, thumb_i) = match (idx("storage_provider"), idx("storage_key"), idx("thumbnail_key")) {
            (Some(p), Some(k), t) => (p, k, t),
            _ => return (Vec::new(), Vec::new()),
        };
        let mut blobs = Vec::new();
        let mut manifest = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for row in &files.rows {
            let provider = row.get(provider_i).and_then(|v| v.clone()).unwrap_or_default();
            let Some(provider_store) = self.storage.get(&provider) else {
                continue;
            };
            let mut keys = Vec::new();
            if let Some(Some(k)) = row.get(key_i) {
                keys.push(k.clone());
            }
            if let Some(ti) = thumb_i {
                if let Some(Some(tk)) = row.get(ti) {
                    keys.push(tk.clone());
                }
            }
            for key in keys {
                if !seen.insert(key.clone()) {
                    continue;
                }
                match provider_store.get(&key).await {
                    Ok(bytes) => {
                        manifest.push(FileManifest {
                            storage_key: key.clone(),
                            provider: provider.clone(),
                            size: bytes.len(),
                        });
                        blobs.push((format!("files/{key}"), bytes.to_vec()));
                    }
                    Err(e) => tracing::warn!(key = %key, error = %e, "backup: skipping unreadable file"),
                }
            }
        }
        (blobs, manifest)
    }

    // --- Restore ---

    /// Inspect a backup artifact's manifest (decrypting/decompressing as needed).
    pub fn inspect(&self, bytes: &[u8]) -> Result<Manifest, BackupError> {
        let (manifest, _, _) = self.open(bytes)?;
        Ok(manifest)
    }

    /// Read a backup artifact from the configured destination.
    pub async fn read_destination(&self, key: &str) -> Result<Vec<u8>, BackupError> {
        let bytes = self
            .destination
            .get(key)
            .await
            .map_err(|e| BackupError::Storage(e.to_string()))?;
        Ok(bytes.to_vec())
    }

    /// Restore from a backup artifact, replacing all data within the chosen scope
    /// in a single transaction.
    pub async fn restore(&self, req: RestoreRequest) -> Result<(), BackupError> {
        let bytes = match &req.source {
            RestoreSource::Bytes(b) => b.clone(),
            RestoreSource::Destination(key) => self.read_destination(key).await?,
        };

        let (manifest, tables_ndjson, file_blobs) = self.open(&bytes)?;

        if manifest.format_version > FORMAT_VERSION {
            return Err(BackupError::Invalid(format!(
                "backup format version {} is newer than supported ({})",
                manifest.format_version, FORMAT_VERSION
            )));
        }
        let current = crate::database::latest_migration_version(self.pool.backend());
        if manifest.schema_version > current {
            return Err(BackupError::SchemaTooNew {
                backup: manifest.schema_version,
                current,
            });
        }

        let mut tables = parse_all_tables(&tables_ndjson)?;

        let plan = match &req.target {
            RestoreTarget::WholeInstance => {
                if manifest.scope != "instance" {
                    return Err(BackupError::Invalid(
                        "whole-instance restore requires an instance backup".into(),
                    ));
                }
                build_instance_plan(self.pool.backend(), &tables)
            }
            RestoreTarget::Site { site_id, import_as_new } => {
                // Extract the single site's subtree if the source is an instance backup.
                if manifest.scope == "instance" {
                    tables = filter_to_site(&tables, site_id);
                }
                let existing = schema::existing_user_ids(&self.pool).await?;
                let fallback = pick_fallback_user(req.created_by.as_deref(), &existing);
                let exists = schema::site_exists(&self.pool, site_id).await?;
                build_site_plan(
                    self.pool.backend(),
                    &mut tables,
                    site_id,
                    exists,
                    *import_as_new,
                    &existing,
                    fallback.as_deref(),
                )
            }
        };

        schema::apply_restore(&self.pool, &plan).await?;

        // Restore file blobs (best-effort: a missing blob is non-fatal).
        self.restore_files(&manifest, &file_blobs).await;
        Ok(())
    }

    async fn restore_files(&self, manifest: &Manifest, blobs: &HashMap<String, Vec<u8>>) {
        for f in &manifest.files {
            let Some(data) = blobs.get(&f.storage_key) else {
                continue;
            };
            let provider = self
                .storage
                .get(&f.provider)
                .or_else(|| self.storage.get("filesystem"))
                .or_else(|| self.storage.get("s3"));
            let Some(provider) = provider else {
                tracing::warn!(key = %f.storage_key, "restore: no storage provider available for file");
                continue;
            };
            if let Err(e) = provider
                .put(
                    &f.storage_key,
                    bytes::Bytes::from(data.clone()),
                    "application/octet-stream",
                )
                .await
            {
                tracing::warn!(key = %f.storage_key, error = %e, "restore: failed to write file");
            }
        }
    }

    // --- Artifact wrapping ---

    fn wrap(&self, compressed: Vec<u8>, encrypt: bool) -> Result<Vec<u8>, BackupError> {
        let mut out = Vec::with_capacity(compressed.len() + 32);
        out.extend_from_slice(MAGIC);
        if encrypt {
            let key = self.encryption_key.ok_or(BackupError::MissingKey)?;
            out.push(FLAG_ENCRYPTED);
            let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
            let mut nonce_bytes = [0u8; 12];
            rand::thread_rng().fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);
            let ciphertext = cipher
                .encrypt(nonce, compressed.as_ref())
                .map_err(|e| BackupError::Crypto(e.to_string()))?;
            out.extend_from_slice(&nonce_bytes);
            out.extend_from_slice(&ciphertext);
        } else {
            out.push(0);
            out.extend_from_slice(&compressed);
        }
        Ok(out)
    }

    /// Decrypt/decompress an artifact into (manifest, table NDJSON, file blobs).
    fn open(
        &self,
        bytes: &[u8],
    ) -> Result<(Manifest, HashMap<String, Vec<u8>>, HashMap<String, Vec<u8>>), BackupError> {
        if bytes.len() < MAGIC.len() + 1 || &bytes[..MAGIC.len()] != MAGIC {
            return Err(BackupError::Invalid("not a CMS backup artifact".into()));
        }
        let flags = bytes[MAGIC.len()];
        let body = &bytes[MAGIC.len() + 1..];

        let compressed = if flags & FLAG_ENCRYPTED != 0 {
            let key = self.encryption_key.ok_or(BackupError::MissingKey)?;
            if body.len() < 12 {
                return Err(BackupError::Invalid("truncated encrypted backup".into()));
            }
            let (nonce_bytes, ciphertext) = body.split_at(12);
            let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
            cipher
                .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
                .map_err(|_| BackupError::Crypto("decryption failed (wrong key?)".into()))?
        } else {
            body.to_vec()
        };

        let tar_bytes = zstd::stream::decode_all(&compressed[..]).map_err(|e| BackupError::Io(e.to_string()))?;
        read_tar(&tar_bytes)
    }
}

// --- Destination construction ----------------------------------------------

/// Build the backup destination storage provider from config (independent of the
/// site-files storage). Falls back to a local `backups/` directory.
pub fn build_backup_destination(config: &Config) -> Result<Arc<dyn StorageProvider>, BackupError> {
    if config.backup_destination == "s3" && config.has_backup_s3() {
        let s3 = S3Storage::new(
            config.backup_s3_access_key_id.as_deref().unwrap(),
            config.backup_s3_secret_access_key.as_deref().unwrap(),
            config.backup_s3_bucket.as_deref().unwrap(),
            config.backup_s3_region.as_deref().unwrap_or("us-east-1"),
            config.backup_s3_endpoint.as_deref(),
            config.backup_s3_public_url.as_deref(),
        )
        .map_err(|e| BackupError::Storage(e.to_string()))?;
        return Ok(Arc::new(s3));
    }
    let path = config
        .backup_local_path
        .clone()
        .unwrap_or_else(|| crate::paths::backups_dir().to_string_lossy().into_owned());
    let fs = FileSystemStorage::new(&path).map_err(|e| BackupError::Storage(e.to_string()))?;
    Ok(Arc::new(fs))
}

// --- Plan building ----------------------------------------------------------

fn build_instance_plan(backend: crate::database::backend::DatabaseBackend, tables: &Tables) -> schema::RestorePlan {
    let deletes = vec![
        schema::Statement {
            sql: "DELETE FROM sites".into(),
            rows: vec![],
        },
        schema::Statement {
            sql: "DELETE FROM users".into(),
            rows: vec![],
        },
    ];
    let inserts = inserts_for(backend, tables);
    schema::RestorePlan { deletes, inserts }
}

#[allow(clippy::too_many_arguments)]
fn build_site_plan(
    backend: crate::database::backend::DatabaseBackend,
    tables: &mut Tables,
    site_id: &str,
    exists: bool,
    import_as_new: bool,
    existing_users: &HashSet<String>,
    fallback_user: Option<&str>,
) -> schema::RestorePlan {
    if import_as_new {
        remap_ids(tables);
    }
    reconcile_user_refs(tables, existing_users, fallback_user);

    let deletes = if !import_as_new && exists {
        let ph = if backend == crate::database::backend::DatabaseBackend::Postgres {
            "$1"
        } else {
            "?"
        };
        vec![schema::Statement {
            sql: format!("DELETE FROM sites WHERE id = {ph}"),
            rows: vec![vec![Some(site_id.to_string())]],
        }]
    } else {
        vec![]
    };
    let inserts = inserts_for(backend, tables);
    schema::RestorePlan { deletes, inserts }
}

/// Build INSERT statements for every present table in FK order, skipping `users`
/// for site scope (it is not part of a site backup).
fn inserts_for(backend: crate::database::backend::DatabaseBackend, tables: &Tables) -> Vec<schema::Statement> {
    let mut stmts = Vec::new();
    for spec in schema::TABLES {
        let Some(rows) = tables.get(spec.name) else {
            continue;
        };
        if rows.is_empty() {
            continue;
        }
        let sql = schema::insert_sql(backend, spec);
        let mut value_rows = Vec::with_capacity(rows.len());
        for m in rows {
            value_rows.push(map_to_values(spec, m));
        }
        stmts.push(schema::Statement { sql, rows: value_rows });
    }
    stmts
}

// --- Restore data transforms ------------------------------------------------

type Row = serde_json::Map<String, serde_json::Value>;
type Tables = HashMap<String, Vec<Row>>;

fn parse_all_tables(ndjson: &HashMap<String, Vec<u8>>) -> Result<Tables, BackupError> {
    let mut tables = Tables::new();
    for (name, bytes) in ndjson {
        tables.insert(name.clone(), parse_ndjson(bytes)?);
    }
    Ok(tables)
}

fn parse_ndjson(bytes: &[u8]) -> Result<Vec<Row>, BackupError> {
    let text = std::str::from_utf8(bytes).map_err(|e| BackupError::Invalid(e.to_string()))?;
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| BackupError::Invalid(e.to_string()))?;
        match v {
            serde_json::Value::Object(m) => rows.push(m),
            _ => return Err(BackupError::Invalid("table row is not a JSON object".into())),
        }
    }
    Ok(rows)
}

fn map_to_values(spec: &schema::TableSpec, m: &Row) -> Vec<Option<String>> {
    spec.columns
        .iter()
        .map(|c| match m.get(c.name) {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            Some(serde_json::Value::Null) | None => None,
            Some(other) => Some(other.to_string()),
        })
        .collect()
}

fn str_field<'a>(m: &'a Row, key: &str) -> Option<&'a str> {
    m.get(key).and_then(|v| v.as_str())
}

/// Keep only the rows belonging to a single site (for extracting one site from an
/// instance backup). Drops the `users` table.
fn filter_to_site(tables: &Tables, site_id: &str) -> Tables {
    let mut out = Tables::new();
    let keep_by = |rows: Option<&Vec<Row>>, key: &str, val: &str| -> Vec<Row> {
        rows.map(|r| r.iter().filter(|m| str_field(m, key) == Some(val)).cloned().collect())
            .unwrap_or_default()
    };

    out.insert("sites".into(), keep_by(tables.get("sites"), "id", site_id));
    for t in [
        "site_members",
        "collections",
        "entries",
        "files",
        "entry_file_references",
        "access_tokens",
        "site_webhooks",
    ] {
        out.insert(t.into(), keep_by(tables.get(t), "site_id", site_id));
    }

    let entry_ids: HashSet<&str> = out
        .get("entries")
        .map(|r| r.iter().filter_map(|m| str_field(m, "id")).collect())
        .unwrap_or_default();
    let revs = tables
        .get("entry_revisions")
        .map(|r| {
            r.iter()
                .filter(|m| str_field(m, "entry_id").is_some_and(|e| entry_ids.contains(e)))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    out.insert("entry_revisions".into(), revs);

    let webhook_ids: HashSet<&str> = out
        .get("site_webhooks")
        .map(|r| r.iter().filter_map(|m| str_field(m, "id")).collect())
        .unwrap_or_default();
    let deliveries = tables
        .get("site_webhook_deliveries")
        .map(|r| {
            r.iter()
                .filter(|m| str_field(m, "webhook_id").is_some_and(|w| webhook_ids.contains(w)))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    out.insert("site_webhook_deliveries".into(), deliveries);
    out
}

/// For site restores (backup has no `users`): drop members whose user is gone,
/// null dangling optional user refs, and fall back `sites.created_by` to the actor.
fn reconcile_user_refs(tables: &mut Tables, existing: &HashSet<String>, fallback_user: Option<&str>) {
    if let Some(members) = tables.get_mut("site_members") {
        members.retain(|m| str_field(m, "user_id").is_some_and(|u| existing.contains(u)));
    }

    let nullable: [(&str, &str); 5] = [
        ("files", "created_by"),
        ("entry_revisions", "created_by"),
        ("access_tokens", "created_by_user_id"),
        ("site_webhooks", "created_by"),
        ("site_webhook_deliveries", "triggered_by"),
    ];
    for (table, col) in nullable {
        if let Some(rows) = tables.get_mut(table) {
            for m in rows.iter_mut() {
                if let Some(u) = str_field(m, col) {
                    if !existing.contains(u) {
                        m.insert(col.to_string(), serde_json::Value::Null);
                    }
                }
            }
        }
    }

    if let Some(sites) = tables.get_mut("sites") {
        for m in sites.iter_mut() {
            let missing = str_field(m, "created_by")
                .map(|u| !existing.contains(u))
                .unwrap_or(true);
            if missing {
                if let Some(fb) = fallback_user {
                    m.insert("created_by".to_string(), serde_json::Value::String(fb.to_string()));
                }
            }
        }
    }
}

/// Remap every primary key (and referencing FK) to a fresh id, so a site is
/// imported as an independent copy. User ids are preserved (reconciled separately).
fn remap_ids(tables: &mut Tables) {
    // Build old->new id maps for tables with their own primary key.
    let pk_tables = [
        "sites",
        "site_members",
        "collections",
        "entries",
        "files",
        "entry_revisions",
        "access_tokens",
        "site_webhooks",
        "site_webhook_deliveries",
    ];
    let mut maps: HashMap<&str, HashMap<String, String>> = HashMap::new();
    for t in pk_tables {
        let mut m = HashMap::new();
        if let Some(rows) = tables.get_mut(t) {
            for row in rows.iter_mut() {
                if let Some(old) = str_field(row, "id").map(String::from) {
                    let new = new_id();
                    m.insert(old, new.clone());
                    row.insert("id".to_string(), serde_json::Value::String(new));
                }
            }
        }
        maps.insert(t, m);
    }

    let remap = |row: &mut Row, col: &str, map: &HashMap<String, String>| {
        if let Some(old) = str_field(row, col).map(String::from) {
            if let Some(new) = map.get(&old) {
                row.insert(col.to_string(), serde_json::Value::String(new.clone()));
            }
        }
    };
    let sites = maps["sites"].clone();
    let collections = maps["collections"].clone();
    let entries = maps["entries"].clone();
    let files = maps["files"].clone();
    let webhooks = maps["site_webhooks"].clone();

    for (table, cols) in [
        ("site_members", vec![("site_id", &sites)]),
        ("collections", vec![("site_id", &sites)]),
        (
            "entries",
            vec![
                ("site_id", &sites),
                ("collection_id", &collections),
                ("singleton_collection_id", &collections),
            ],
        ),
        ("files", vec![("site_id", &sites)]),
        (
            "entry_file_references",
            vec![("site_id", &sites), ("entry_id", &entries), ("file_id", &files)],
        ),
        ("entry_revisions", vec![("entry_id", &entries)]),
        ("access_tokens", vec![("site_id", &sites)]),
        ("site_webhooks", vec![("site_id", &sites)]),
        ("site_webhook_deliveries", vec![("webhook_id", &webhooks)]),
    ] {
        if let Some(rows) = tables.get_mut(table) {
            for row in rows.iter_mut() {
                for &(col, map) in &cols {
                    remap(row, col, map);
                }
            }
        }
    }
}

fn pick_fallback_user(actor: Option<&str>, existing: &HashSet<String>) -> Option<String> {
    if let Some(a) = actor {
        if existing.contains(a) {
            return Some(a.to_string());
        }
    }
    existing.iter().next().cloned()
}

// --- Tar / ndjson helpers ---------------------------------------------------

fn table_to_ndjson(t: &schema::DumpedTable) -> Vec<u8> {
    let mut buf = Vec::new();
    for row in &t.rows {
        let mut obj = serde_json::Map::new();
        for (i, name) in t.columns.iter().enumerate() {
            let v = match row.get(i).and_then(|x| x.as_ref()) {
                Some(s) => serde_json::Value::String(s.clone()),
                None => serde_json::Value::Null,
            };
            obj.insert((*name).to_string(), v);
        }
        if let Ok(line) = serde_json::to_string(&serde_json::Value::Object(obj)) {
            buf.extend_from_slice(line.as_bytes());
            buf.push(b'\n');
        }
    }
    buf
}

fn build_tar(
    manifest: &Manifest,
    tables: &[(String, Vec<u8>)],
    files: &[(String, Vec<u8>)],
) -> Result<Vec<u8>, BackupError> {
    let mut builder = tar::Builder::new(Vec::new());
    let manifest_json = serde_json::to_vec_pretty(manifest).map_err(|e| BackupError::Io(e.to_string()))?;
    append_entry(&mut builder, "manifest.json", &manifest_json)?;
    for (path, data) in tables {
        append_entry(&mut builder, path, data)?;
    }
    for (path, data) in files {
        append_entry(&mut builder, path, data)?;
    }
    builder.into_inner().map_err(|e| BackupError::Io(e.to_string()))
}

fn append_entry(builder: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8]) -> Result<(), BackupError> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();
    builder
        .append_data(&mut header, path, data)
        .map_err(|e| BackupError::Io(e.to_string()))
}

fn read_tar(tar_bytes: &[u8]) -> Result<(Manifest, HashMap<String, Vec<u8>>, HashMap<String, Vec<u8>>), BackupError> {
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let mut manifest: Option<Manifest> = None;
    let mut tables = HashMap::new();
    let mut files = HashMap::new();

    for entry in archive.entries().map_err(|e| BackupError::Io(e.to_string()))? {
        let mut entry = entry.map_err(|e| BackupError::Io(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| BackupError::Io(e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| BackupError::Io(e.to_string()))?;

        if path == "manifest.json" {
            manifest = Some(serde_json::from_slice(&data).map_err(|e| BackupError::Invalid(e.to_string()))?);
        } else if let Some(name) = path.strip_prefix("tables/").and_then(|p| p.strip_suffix(".ndjson")) {
            tables.insert(name.to_string(), data);
        } else if let Some(key) = path.strip_prefix("files/") {
            files.insert(key.to_string(), data);
        }
    }

    let manifest = manifest.ok_or_else(|| BackupError::Invalid("backup missing manifest.json".into()))?;
    Ok((manifest, tables, files))
}

// --- small utilities --------------------------------------------------------

pub fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn parse_key_hex(hex_str: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(hex_str).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}
