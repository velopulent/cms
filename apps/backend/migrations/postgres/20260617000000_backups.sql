-- Backup & restore: schedules, backup artifacts, and restore audit log.
-- These tables are instance-local bookkeeping and are intentionally NOT part of
-- the backup payload itself. Integer columns are BIGINT so the application can
-- bind i64 uniformly across all backends.

CREATE TABLE IF NOT EXISTS backup_schedules (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL CHECK(scope IN ('instance', 'site')),
    site_id TEXT REFERENCES sites(id) ON DELETE CASCADE,
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
