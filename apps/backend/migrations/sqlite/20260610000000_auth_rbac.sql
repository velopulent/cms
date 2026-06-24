ALTER TABLE users ADD COLUMN instance_role TEXT CHECK(instance_role IS NULL OR instance_role = 'instance_owner');
ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;

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

UPDATE users
SET instance_role = 'instance_owner'
WHERE id = COALESCE(
    (SELECT id FROM users WHERE email = 'admin@cms.local'),
    (SELECT id FROM users ORDER BY created_at, id LIMIT 1)
)
AND NOT EXISTS (SELECT 1 FROM users WHERE instance_role = 'instance_owner');
