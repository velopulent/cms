-- CMS Schema (SQLite)

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    instance_role TEXT CHECK(instance_role IS NULL OR instance_role IN ('instance_owner', 'instance_admin')),
    must_change_password INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS storage_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK(kind IN ('filesystem', 's3')),
    endpoint TEXT,
    region TEXT,
    bucket TEXT,
    public_url TEXT,
    credentials_encrypted TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    immutable INTEGER NOT NULL DEFAULT 0,
    created_by TEXT REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO storage_profiles(id, name, kind, enabled, immutable)
VALUES ('local-filesystem', 'Local Filesystem', 'filesystem', 1, 1);

CREATE TABLE IF NOT EXISTS sites (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    storage_provider TEXT NOT NULL DEFAULT 'filesystem',
    storage_profile_id TEXT NOT NULL DEFAULT 'local-filesystem' REFERENCES storage_profiles(id),
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS site_members (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('editor', 'viewer')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(site_id, user_id)
);

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    definition JSON NOT NULL,
    is_singleton INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (id, site_id)
);

CREATE TABLE IF NOT EXISTS entries (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    collection_id TEXT NOT NULL,
    data JSON NOT NULL,
    slug TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'published')),
    singleton_collection_id TEXT REFERENCES collections(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    published_at TEXT,
    -- Opaque optimistic-concurrency version; bumped by entries_version on every update.
    version INTEGER NOT NULL DEFAULT 1,
    UNIQUE (id, site_id),
    FOREIGN KEY (collection_id, site_id) REFERENCES collections(id, site_id) ON DELETE CASCADE,
    CHECK (singleton_collection_id IS NULL OR singleton_collection_id = collection_id)
);

CREATE TRIGGER IF NOT EXISTS entries_version AFTER UPDATE ON entries
WHEN NEW.version = OLD.version
BEGIN
    UPDATE entries SET version = OLD.version + 1 WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS entries_singleton_insert BEFORE INSERT ON entries
WHEN NEW.singleton_collection_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM collections WHERE id = NEW.singleton_collection_id AND is_singleton = 1
)
BEGIN
    SELECT RAISE(ABORT, 'invalid singleton collection');
END;

CREATE INDEX IF NOT EXISTS idx_site_members_user ON site_members(user_id);
CREATE INDEX IF NOT EXISTS idx_site_members_site ON site_members(site_id);
CREATE INDEX IF NOT EXISTS idx_collections_site ON collections(site_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_collections_site_name ON collections(site_id, name);
CREATE UNIQUE INDEX IF NOT EXISTS idx_collections_site_slug ON collections(site_id, slug);
CREATE INDEX IF NOT EXISTS idx_entries_site ON entries(site_id);
CREATE INDEX IF NOT EXISTS idx_entries_slug ON entries(slug);
CREATE INDEX IF NOT EXISTS idx_entries_collection ON entries(collection_id);
CREATE INDEX IF NOT EXISTS idx_entries_status ON entries(status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_entries_collection_slug ON entries(collection_id, slug);
CREATE UNIQUE INDEX IF NOT EXISTS uniq_singleton_entry ON entries(singleton_collection_id) WHERE singleton_collection_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS access_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_prefix TEXT NOT NULL,
    token_hmac TEXT NOT NULL UNIQUE,
    scopes_json TEXT NOT NULL,
    created_by_user_id TEXT REFERENCES users(id),
    last_used_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_access_tokens_prefix ON access_tokens(token_prefix);
CREATE INDEX IF NOT EXISTS idx_access_tokens_site ON access_tokens(site_id);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    original_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    storage_provider TEXT NOT NULL CHECK(storage_provider IN ('filesystem', 's3')),
    storage_key TEXT NOT NULL,
    thumbnail_key TEXT,
    width INTEGER,
    height INTEGER,
    deleted_at TEXT,
    created_by TEXT REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (id, site_id)
);

CREATE INDEX IF NOT EXISTS idx_files_site ON files(site_id);
CREATE INDEX IF NOT EXISTS idx_files_created_by ON files(created_by);

CREATE TABLE IF NOT EXISTS entry_file_references (
    entry_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    field_name TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (entry_id, file_id, field_name),
    FOREIGN KEY (entry_id, site_id) REFERENCES entries(id, site_id) ON DELETE CASCADE,
    FOREIGN KEY (file_id, site_id) REFERENCES files(id, site_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_efr_file ON entry_file_references(file_id);
CREATE INDEX IF NOT EXISTS idx_efr_entry ON entry_file_references(entry_id);

CREATE TABLE IF NOT EXISTS entry_revisions (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    revision_number INTEGER NOT NULL,
    data TEXT NOT NULL,
    created_by TEXT REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    change_summary TEXT,
    UNIQUE(entry_id, revision_number)
);

CREATE INDEX IF NOT EXISTS idx_entry_revisions_entry_id ON entry_revisions(entry_id);
CREATE INDEX IF NOT EXISTS idx_entry_revisions_entry_number ON entry_revisions(entry_id, revision_number);
CREATE INDEX IF NOT EXISTS idx_entry_revisions_created_at ON entry_revisions(entry_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_webhooks (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    url TEXT NOT NULL,
    headers_encrypted TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS site_webhook_deliveries (
    id TEXT PRIMARY KEY NOT NULL,
    webhook_id TEXT NOT NULL REFERENCES site_webhooks(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK(status IN ('success', 'failed')),
    status_code INTEGER,
    response_body TEXT,
    duration_ms INTEGER,
    triggered_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    triggered_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_site_webhooks_site_id ON site_webhooks(site_id);
CREATE INDEX IF NOT EXISTS idx_site_webhook_deliveries_webhook_id ON site_webhook_deliveries(webhook_id);

CREATE INDEX IF NOT EXISTS idx_files_site_mime ON files(site_id, mime_type);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    csrf_token_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at TEXT
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

CREATE TABLE security_audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    target_type TEXT,
    target_id TEXT,
    site_id TEXT REFERENCES sites(id) ON DELETE SET NULL,
    ip_address TEXT,
    user_agent TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_security_audit_actor ON security_audit_events(actor_user_id);
CREATE INDEX idx_security_audit_site ON security_audit_events(site_id);
CREATE INDEX idx_security_audit_created ON security_audit_events(created_at);

CREATE INDEX IF NOT EXISTS idx_entries_site_status ON entries(site_id, status);
CREATE INDEX IF NOT EXISTS idx_files_site_deleted ON files(site_id, deleted_at);

-- Backup & restore: schedules, backup artifacts, and restore audit log.
-- These tables are instance-local bookkeeping and are intentionally NOT part of
-- the backup payload itself.

CREATE TABLE IF NOT EXISTS backup_schedules (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL CHECK(scope IN ('instance', 'site')),
    site_id TEXT REFERENCES sites(id) ON DELETE CASCADE,
    storage_profile_id TEXT REFERENCES storage_profiles(id),
    cron TEXT NOT NULL,
    retention_n INTEGER NOT NULL DEFAULT 7,
    include_files INTEGER NOT NULL DEFAULT 1,
    encrypt INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_run_at TEXT,
    next_run_at TEXT,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK ((scope = 'instance' AND site_id IS NULL) OR (scope = 'site' AND site_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_backup_schedules_site ON backup_schedules(site_id);
CREATE INDEX IF NOT EXISTS idx_backup_schedules_due ON backup_schedules(enabled, next_run_at);

CREATE TABLE IF NOT EXISTS backups (
    id TEXT PRIMARY KEY NOT NULL,
    schedule_id TEXT REFERENCES backup_schedules(id) ON DELETE SET NULL,
    scope TEXT NOT NULL CHECK(scope IN ('instance', 'site')),
    site_id TEXT,
    storage_profile_id TEXT REFERENCES storage_profiles(id),
    status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'success', 'failed')),
    format_version INTEGER NOT NULL DEFAULT 1,
    schema_version TEXT,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,
    includes_files INTEGER NOT NULL DEFAULT 0,
    encrypted INTEGER NOT NULL DEFAULT 0,
    destination_key TEXT,
    checksum TEXT,
    error TEXT,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK ((scope = 'instance' AND site_id IS NULL) OR (scope = 'site' AND site_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_backups_schedule ON backups(schedule_id);
CREATE INDEX IF NOT EXISTS idx_backups_scope ON backups(scope, site_id);
CREATE INDEX IF NOT EXISTS idx_backups_created ON backups(created_at DESC);

CREATE TABLE IF NOT EXISTS restore_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL,
    scope TEXT NOT NULL CHECK(scope IN ('instance', 'site')),
    target_site_id TEXT REFERENCES sites(id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'success', 'failed')),
    error TEXT,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK ((scope = 'instance' AND target_site_id IS NULL) OR (scope = 'site' AND target_site_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_restore_jobs_created ON restore_jobs(created_at DESC);

-- Cross-process search index queue.
--
-- Any process that writes entry content (server, gRPC, MCP HTTP, or a separate
-- `cms mcp stdio` process) enqueues a row here. The single running server owns
-- the Tantivy IndexWriter and is the sole consumer: it drains this queue, applies
-- the changes to the index, and deletes the processed rows. This makes search
-- sync work across processes and survive restarts (the queue is durable), while
-- keeping the embedded single-writer model.
--
-- `id` is a UUIDv7 (time-ordered) so draining in `id` order is chronological.
-- `op` is advisory; the consumer re-derives the action by looking the entry up in
-- the database (present => upsert, absent => delete).

-- NOTE: Foreign keys on entry_id/site_id are intentionally omitted so the queue
-- survives entry/site deletion â€” stale rows are harmless and cleaned up on drain.
CREATE TABLE IF NOT EXISTS search_index_queue (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL,
    site_id TEXT NOT NULL,
    op TEXT NOT NULL CHECK(op IN ('index', 'delete')),
    enqueued_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_search_index_queue_entry ON search_index_queue(entry_id);
CREATE INDEX IF NOT EXISTS idx_search_index_queue_enqueued ON search_index_queue(enqueued_at);

CREATE TABLE instance_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL,
    settings_json TEXT NOT NULL,
    credentials_encrypted TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS personal_access_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_prefix TEXT NOT NULL,
    token_hmac TEXT NOT NULL UNIQUE,
    scopes_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at TEXT,
    expires_at TEXT,
    revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_pat_prefix ON personal_access_tokens(token_prefix);

CREATE TABLE IF NOT EXISTS deployment_triggers (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    provider TEXT NOT NULL,
    url_encrypted TEXT NOT NULL,
    headers_encrypted TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    is_primary INTEGER NOT NULL DEFAULT 0,
    cooldown_seconds INTEGER NOT NULL DEFAULT 60,
    daily_quota INTEGER NOT NULL DEFAULT 20,
    created_by TEXT REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_primary
    ON deployment_triggers(site_id) WHERE is_primary = 1;

CREATE TABLE IF NOT EXISTS deployment_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    trigger_id TEXT NOT NULL REFERENCES deployment_triggers(id) ON DELETE CASCADE,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    status_code INTEGER,
    error_category TEXT,
    response_body TEXT,
    retry_after_seconds INTEGER,
    duration_ms INTEGER,
    triggered_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_deployment_jobs_trigger
    ON deployment_jobs(trigger_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_jobs_active_trigger
    ON deployment_jobs(trigger_id) WHERE status IN ('queued', 'running');

-- Single-use record for signed upload URLs. Security state, not content:
-- deliberately excluded from logical backups.
CREATE TABLE IF NOT EXISTS signed_upload_uses (
    file_id TEXT PRIMARY KEY,
    expires_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_signed_upload_uses_expiry ON signed_upload_uses(expires_at);
