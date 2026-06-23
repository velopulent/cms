ALTER TABLE users
    ADD COLUMN instance_role VARCHAR(32) NULL,
    ADD COLUMN must_change_password BOOLEAN NOT NULL DEFAULT FALSE,
    ADD CONSTRAINT chk_users_instance_role CHECK(instance_role IS NULL OR instance_role = 'instance_owner');

CREATE TABLE sessions (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    user_id VARCHAR(36) NOT NULL,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    csrf_token_hash VARCHAR(64) NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    last_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at DATETIME NULL,
    CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);

CREATE TABLE security_audit_events (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    actor_user_id VARCHAR(36) NULL,
    event_type VARCHAR(100) NOT NULL,
    target_type VARCHAR(100) NULL,
    target_id VARCHAR(255) NULL,
    site_id VARCHAR(36) NULL,
    ip_address VARCHAR(64) NULL,
    user_agent TEXT NULL,
    metadata JSON NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_audit_user FOREIGN KEY (actor_user_id) REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT fk_audit_site FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE SET NULL
);

CREATE INDEX idx_security_audit_actor ON security_audit_events(actor_user_id);
CREATE INDEX idx_security_audit_site ON security_audit_events(site_id);
CREATE INDEX idx_security_audit_created ON security_audit_events(created_at);

UPDATE users
SET instance_role = 'instance_owner'
WHERE id = COALESCE(
    (SELECT selected.id FROM (SELECT id FROM users WHERE email = 'admin@cms.local') selected),
    (SELECT selected.id FROM (SELECT id FROM users ORDER BY created_at, id LIMIT 1) selected)
)
AND NOT EXISTS (
    SELECT 1 FROM (SELECT instance_role FROM users) existing WHERE existing.instance_role = 'instance_owner'
);
