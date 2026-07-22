CREATE TABLE IF NOT EXISTS personal_access_tokens (
    id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL, token_hash TEXT NOT NULL, token_hmac TEXT NOT NULL,
    token_prefix TEXT NOT NULL, scopes_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')), last_used_at TEXT,
    expires_at TEXT, revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_pat_prefix ON personal_access_tokens(token_prefix);
ALTER TABLE access_tokens ADD COLUMN scopes_json TEXT NOT NULL DEFAULT '[]';

CREATE TABLE IF NOT EXISTS storage_profiles (
    id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, kind TEXT NOT NULL CHECK(kind IN ('filesystem','s3')),
    endpoint TEXT, region TEXT, bucket TEXT, public_url TEXT, credentials_encrypted TEXT,
    enabled INTEGER NOT NULL DEFAULT 1, immutable INTEGER NOT NULL DEFAULT 0,
    created_by TEXT REFERENCES users(id), created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT OR IGNORE INTO storage_profiles(id,name,kind,enabled,immutable) VALUES ('local-filesystem','Local Filesystem','filesystem',1,1);
ALTER TABLE sites ADD COLUMN storage_profile_id TEXT REFERENCES storage_profiles(id);
UPDATE sites SET storage_profile_id = 'local-filesystem' WHERE storage_profile_id IS NULL;
ALTER TABLE backups ADD COLUMN storage_profile_id TEXT REFERENCES storage_profiles(id);
ALTER TABLE backup_schedules ADD COLUMN storage_profile_id TEXT REFERENCES storage_profiles(id);

CREATE TABLE IF NOT EXISTS deployment_triggers (
    id TEXT PRIMARY KEY, site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    label TEXT NOT NULL, provider TEXT NOT NULL, url_encrypted TEXT NOT NULL, headers_encrypted TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1, is_primary INTEGER NOT NULL DEFAULT 0,
    cooldown_seconds INTEGER NOT NULL DEFAULT 60, daily_quota INTEGER NOT NULL DEFAULT 20,
    created_by TEXT REFERENCES users(id), created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_primary ON deployment_triggers(site_id) WHERE is_primary = 1;
CREATE TABLE IF NOT EXISTS deployment_jobs (
    id TEXT PRIMARY KEY, trigger_id TEXT NOT NULL REFERENCES deployment_triggers(id) ON DELETE CASCADE,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE, status TEXT NOT NULL,
    status_code INTEGER, error_category TEXT, response_body TEXT, retry_after_seconds INTEGER,
    duration_ms INTEGER, triggered_by TEXT, created_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT, finished_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_deployment_jobs_trigger ON deployment_jobs(trigger_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_deployment_jobs_active_trigger
    ON deployment_jobs(trigger_id) WHERE status IN ('queued', 'running');
