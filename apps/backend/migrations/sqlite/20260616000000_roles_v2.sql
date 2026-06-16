-- no-transaction
-- Roles v2: instance operators (owner/admin) + site collaborators (editor/viewer).
-- SQLite cannot ALTER a CHECK constraint in place, so both affected tables are rebuilt.
-- Run outside a transaction so foreign_keys can be toggled off for the rebuild.

PRAGMA foreign_keys=OFF;

-- Widen users.instance_role to allow instance_admin.
CREATE TABLE users_new (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    instance_role TEXT CHECK(instance_role IS NULL OR instance_role IN ('instance_owner', 'instance_admin')),
    must_change_password INTEGER NOT NULL DEFAULT 0
);
INSERT INTO users_new (id, username, email, password_hash, created_at, updated_at, instance_role, must_change_password)
    SELECT id, username, email, password_hash, created_at, updated_at, instance_role, must_change_password FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- Restrict site_members.role to editor/viewer; legacy owner/admin collapse to editor
-- (those operators now act through their instance role, not site membership).
CREATE TABLE site_members_new (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('editor', 'viewer')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(site_id, user_id)
);
INSERT INTO site_members_new (id, site_id, user_id, role, created_at)
    SELECT id, site_id, user_id,
        CASE WHEN role IN ('owner', 'admin') THEN 'editor' ELSE role END,
        created_at
    FROM site_members;
DROP TABLE site_members;
ALTER TABLE site_members_new RENAME TO site_members;

CREATE INDEX IF NOT EXISTS idx_site_members_user ON site_members(user_id);
CREATE INDEX IF NOT EXISTS idx_site_members_site ON site_members(site_id);

PRAGMA foreign_keys=ON;
