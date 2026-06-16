-- Backup & restore: schedules, backup artifacts, and restore audit log.
-- These tables are instance-local bookkeeping and are intentionally NOT part of
-- the backup payload itself. Integer columns are BIGINT so the application can
-- bind i64 uniformly across all backends.

CREATE TABLE IF NOT EXISTS backup_schedules (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    scope VARCHAR(20) NOT NULL,
    site_id VARCHAR(36) NULL,
    cron VARCHAR(255) NOT NULL,
    retention_n BIGINT NOT NULL DEFAULT 7,
    include_files BIGINT NOT NULL DEFAULT 1,
    encrypt BIGINT NOT NULL DEFAULT 0,
    enabled BIGINT NOT NULL DEFAULT 1,
    last_run_at VARCHAR(40) NULL,
    next_run_at VARCHAR(40) NULL,
    created_by VARCHAR(36) NULL,
    created_at VARCHAR(40) NOT NULL,
    updated_at VARCHAR(40) NOT NULL,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL,
    CHECK (scope IN ('instance', 'site'))
);

CREATE INDEX idx_backup_schedules_site ON backup_schedules(site_id);
CREATE INDEX idx_backup_schedules_due ON backup_schedules(enabled, next_run_at);

CREATE TABLE IF NOT EXISTS backups (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    schedule_id VARCHAR(36) NULL,
    scope VARCHAR(20) NOT NULL,
    site_id VARCHAR(36) NULL,
    status VARCHAR(20) NOT NULL,
    format_version BIGINT NOT NULL DEFAULT 1,
    schema_version VARCHAR(40) NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    file_count BIGINT NOT NULL DEFAULT 0,
    includes_files BIGINT NOT NULL DEFAULT 0,
    encrypted BIGINT NOT NULL DEFAULT 0,
    destination_key TEXT,
    checksum VARCHAR(128) NULL,
    error TEXT,
    created_by VARCHAR(36) NULL,
    started_at VARCHAR(40) NULL,
    completed_at VARCHAR(40) NULL,
    created_at VARCHAR(40) NOT NULL,
    FOREIGN KEY (schedule_id) REFERENCES backup_schedules(id) ON DELETE SET NULL,
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL,
    CHECK (scope IN ('instance', 'site')),
    CHECK (status IN ('pending', 'running', 'success', 'failed'))
);

CREATE INDEX idx_backups_schedule ON backups(schedule_id);
CREATE INDEX idx_backups_scope ON backups(scope, site_id);
CREATE INDEX idx_backups_created ON backups(created_at);

CREATE TABLE IF NOT EXISTS restore_jobs (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    source VARCHAR(64) NOT NULL,
    scope VARCHAR(20) NOT NULL,
    target_site_id VARCHAR(36) NULL,
    status VARCHAR(20) NOT NULL,
    error TEXT,
    created_by VARCHAR(36) NULL,
    started_at VARCHAR(40) NULL,
    completed_at VARCHAR(40) NULL,
    created_at VARCHAR(40) NOT NULL,
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL,
    CHECK (status IN ('pending', 'running', 'success', 'failed'))
);

CREATE INDEX idx_restore_jobs_created ON restore_jobs(created_at);
