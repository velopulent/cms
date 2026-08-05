-- CMS Schema (PostgreSQL)

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    instance_role TEXT CHECK(instance_role IS NULL OR instance_role IN ('instance_owner', 'instance_admin')),
    must_change_password BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
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
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    immutable BOOLEAN NOT NULL DEFAULT FALSE,
    created_by TEXT REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

INSERT INTO storage_profiles(id, name, kind, enabled, immutable)
VALUES ('local-filesystem', 'Local Filesystem', 'filesystem', TRUE, TRUE)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS sites (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    storage_provider TEXT NOT NULL DEFAULT 'filesystem',
    storage_profile_id TEXT NOT NULL DEFAULT 'local-filesystem' REFERENCES storage_profiles(id),
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS site_members (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('editor', 'viewer')),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(site_id, user_id)
);

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    definition JSONB NOT NULL,
    is_singleton BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS entries (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    data JSONB NOT NULL,
    slug TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'published')),
    singleton_collection_id TEXT REFERENCES collections(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    published_at TIMESTAMP WITH TIME ZONE,
    CONSTRAINT entries_singleton_consistency CHECK (singleton_collection_id IS NULL OR singleton_collection_id = collection_id)
);

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
    last_used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,
    revoked_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX IF NOT EXISTS idx_access_tokens_prefix ON access_tokens(token_prefix);
CREATE INDEX IF NOT EXISTS idx_access_tokens_site ON access_tokens(site_id);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    original_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size BIGINT NOT NULL,
    storage_provider TEXT NOT NULL CHECK(storage_provider IN ('filesystem', 's3')),
    storage_key TEXT NOT NULL,
    thumbnail_key TEXT,
    width INTEGER,
    height INTEGER,
    deleted_at TIMESTAMP WITH TIME ZONE,
    created_by TEXT REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_files_site ON files(site_id);
CREATE INDEX IF NOT EXISTS idx_files_created_by ON files(created_by);

CREATE TABLE IF NOT EXISTS entry_file_references (
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    PRIMARY KEY (entry_id, file_id)
);
CREATE INDEX IF NOT EXISTS idx_efr_file ON entry_file_references(file_id);
CREATE INDEX IF NOT EXISTS idx_efr_entry ON entry_file_references(entry_id);

CREATE TABLE IF NOT EXISTS entry_revisions (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    revision_number BIGINT NOT NULL,
    data JSONB NOT NULL,
    created_by TEXT REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    change_summary TEXT,
    UNIQUE(entry_id, revision_number)
);

CREATE INDEX IF NOT EXISTS idx_entry_revisions_entry_id ON entry_revisions(entry_id);
CREATE INDEX IF NOT EXISTS idx_entry_revisions_entry_number ON entry_revisions(entry_id, revision_number);
CREATE INDEX IF NOT EXISTS idx_entry_revisions_created_at ON entry_revisions(entry_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_webhooks (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label VARCHAR(255) NOT NULL,
    url VARCHAR(2048) NOT NULL,
    headers_encrypted TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_by VARCHAR(36) REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS site_webhook_deliveries (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    webhook_id VARCHAR(36) NOT NULL REFERENCES site_webhooks(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL CHECK(status IN ('success', 'failed')),
    status_code INTEGER,
    response_body TEXT,
    duration_ms BIGINT,
    triggered_by VARCHAR(36) REFERENCES users(id) ON DELETE SET NULL,
    triggered_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_site_webhooks_site_id ON site_webhooks(site_id);
CREATE INDEX IF NOT EXISTS idx_site_webhook_deliveries_webhook_id ON site_webhook_deliveries(webhook_id);

CREATE INDEX IF NOT EXISTS idx_files_site_mime ON files(site_id, mime_type);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    csrf_token_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ
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
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_security_audit_actor ON security_audit_events(actor_user_id);
CREATE INDEX idx_security_audit_site ON security_audit_events(site_id);
CREATE INDEX idx_security_audit_created ON security_audit_events(created_at);

CREATE INDEX IF NOT EXISTS idx_entries_site_status ON entries(site_id, status);
CREATE INDEX IF NOT EXISTS idx_files_site_deleted ON files(site_id, deleted_at);

-- Backup & restore: schedules, backup artifacts, and restore audit log.
-- These tables are instance-local bookkeeping and are intentionally NOT part of
-- the backup payload itself. Integer columns are BIGINT so the application can
-- bind i64 uniformly across all backends.

CREATE TABLE IF NOT EXISTS backup_schedules (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL CHECK(scope IN ('instance', 'site')),
    site_id TEXT REFERENCES sites(id) ON DELETE CASCADE,
    storage_profile_id TEXT REFERENCES storage_profiles(id),
    cron TEXT NOT NULL,
    retention_n BIGINT NOT NULL DEFAULT 7,
    include_files BIGINT NOT NULL DEFAULT 1,
    encrypt BIGINT NOT NULL DEFAULT 0,
    enabled BIGINT NOT NULL DEFAULT 1,
    last_run_at TEXT,
    next_run_at TEXT,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (NOW()::text),
    updated_at TEXT NOT NULL DEFAULT (NOW()::text),
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
    format_version BIGINT NOT NULL DEFAULT 1,
    schema_version TEXT,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    file_count BIGINT NOT NULL DEFAULT 0,
    includes_files BIGINT NOT NULL DEFAULT 0,
    encrypted BIGINT NOT NULL DEFAULT 0,
    destination_key TEXT,
    checksum TEXT,
    error TEXT,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (NOW()::text),
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
    created_at TEXT NOT NULL DEFAULT (NOW()::text),
    CHECK ((scope = 'instance' AND target_site_id IS NULL) OR (scope = 'site' AND target_site_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_restore_jobs_created ON restore_jobs(created_at DESC);

-- Cross-process search index queue. See the sqlite migration for the rationale.
-- `id` is a UUIDv7 (time-ordered); the single server process drains this queue
-- and applies changes to the Tantivy index.

-- NOTE: Foreign keys on entry_id/site_id are intentionally omitted so the queue
-- survives entry/site deletion â€” stale rows are harmless and cleaned up on drain.
CREATE TABLE IF NOT EXISTS search_index_queue (
    id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL,
    site_id TEXT NOT NULL,
    op TEXT NOT NULL CHECK(op IN ('index', 'delete')),
    enqueued_at TEXT NOT NULL DEFAULT (NOW()::text)
);

CREATE INDEX IF NOT EXISTS idx_search_index_queue_entry ON search_index_queue(entry_id);
CREATE INDEX IF NOT EXISTS idx_search_index_queue_enqueued ON search_index_queue(enqueued_at);

CREATE TABLE instance_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL,
    settings_json TEXT NOT NULL,
    credentials_encrypted TEXT,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS personal_access_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_prefix TEXT NOT NULL,
    token_hmac TEXT NOT NULL UNIQUE,
    scopes_json TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_pat_prefix ON personal_access_tokens(token_prefix);

CREATE TABLE IF NOT EXISTS deployment_triggers (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    provider TEXT NOT NULL,
    url_encrypted TEXT NOT NULL,
    headers_encrypted TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    cooldown_seconds BIGINT NOT NULL DEFAULT 60,
    daily_quota BIGINT NOT NULL DEFAULT 20,
    created_by TEXT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_primary
    ON deployment_triggers(site_id) WHERE is_primary = TRUE;

CREATE TABLE IF NOT EXISTS deployment_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    trigger_id TEXT NOT NULL REFERENCES deployment_triggers(id) ON DELETE CASCADE,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    status_code INTEGER,
    error_category TEXT,
    response_body TEXT,
    retry_after_seconds BIGINT,
    duration_ms BIGINT,
    triggered_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_deployment_jobs_trigger
    ON deployment_jobs(trigger_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_jobs_active_trigger
    ON deployment_jobs(trigger_id) WHERE status IN ('queued', 'running');
