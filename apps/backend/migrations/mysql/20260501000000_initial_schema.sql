-- CMS Schema (MySQL)

CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    username VARCHAR(255) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS sites (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    name VARCHAR(255) NOT NULL,
    storage_provider VARCHAR(50) NOT NULL DEFAULT 'filesystem',
    created_by VARCHAR(36) NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS site_members (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    user_id VARCHAR(36) NOT NULL,
    role VARCHAR(20) NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE KEY unique_site_user (site_id, user_id),
    CHECK (role IN ('owner', 'admin', 'editor', 'viewer'))
);

CREATE TABLE IF NOT EXISTS collections (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    definition JSON NOT NULL,
    is_singleton TINYINT(1) NOT NULL DEFAULT 0,
    singleton_data TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS entries (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    collection_id VARCHAR(36) NOT NULL,
    data JSON NOT NULL,
    slug VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    published_at DATETIME,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
    CHECK (status IN ('draft', 'published'))
);

CREATE INDEX idx_site_members_user ON site_members(user_id);
CREATE INDEX idx_site_members_site ON site_members(site_id);
CREATE INDEX idx_collections_site ON collections(site_id);
CREATE UNIQUE INDEX idx_collections_site_name ON collections(site_id, name);
CREATE UNIQUE INDEX idx_collections_site_slug ON collections(site_id, slug);
CREATE INDEX idx_entries_site ON entries(site_id);
CREATE INDEX idx_entries_slug ON entries(slug);
CREATE INDEX idx_entries_collection ON entries(collection_id);
CREATE INDEX idx_entries_status ON entries(status);
CREATE UNIQUE INDEX idx_entries_collection_slug ON entries(collection_id, slug);

CREATE TABLE IF NOT EXISTS access_tokens (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    name VARCHAR(255) NOT NULL,
    token_hash TEXT NOT NULL,
    token_prefix VARCHAR(64) NOT NULL,
    token_hmac TEXT,
    permission VARCHAR(20) NOT NULL,
    created_by_user_id VARCHAR(36),
    last_used_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME,
    revoked_at DATETIME,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id),
    CHECK (permission IN ('read', 'write'))
);

CREATE UNIQUE INDEX idx_access_tokens_hash ON access_tokens(token_hash);
CREATE INDEX idx_access_tokens_prefix ON access_tokens(token_prefix);
CREATE INDEX idx_access_tokens_site ON access_tokens(site_id);

CREATE TABLE IF NOT EXISTS files (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    filename VARCHAR(255) NOT NULL,
    original_name VARCHAR(255) NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    size BIGINT NOT NULL,
    storage_provider VARCHAR(50) NOT NULL,
    storage_key TEXT NOT NULL,
    thumbnail_key TEXT,
    width INT,
    height INT,
    deleted_at DATETIME,
    created_by VARCHAR(36),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id),
    CHECK (storage_provider IN ('filesystem', 's3'))
);

CREATE INDEX idx_files_site ON files(site_id);
CREATE INDEX idx_files_created_by ON files(created_by);

CREATE TABLE IF NOT EXISTS entry_file_references (
    entry_id VARCHAR(36) NOT NULL,
    file_id VARCHAR(36) NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    PRIMARY KEY (entry_id, file_id),
    FOREIGN KEY (entry_id) REFERENCES entries(id) ON DELETE CASCADE,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE
);
CREATE INDEX idx_efr_file ON entry_file_references(file_id);
CREATE INDEX idx_efr_entry ON entry_file_references(entry_id);

CREATE TABLE IF NOT EXISTS entry_revisions (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    entry_id VARCHAR(36) NOT NULL,
    revision_number INTEGER NOT NULL,
    data JSON NOT NULL,
    created_by VARCHAR(36),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    change_summary TEXT,
    FOREIGN KEY (entry_id) REFERENCES entries(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id),
    UNIQUE KEY unique_entry_revision (entry_id, revision_number)
);

CREATE INDEX idx_entry_revisions_entry_id ON entry_revisions(entry_id);
CREATE INDEX idx_entry_revisions_entry_number ON entry_revisions(entry_id, revision_number);
CREATE INDEX idx_entry_revisions_created_at ON entry_revisions(entry_id, created_at DESC);

CREATE TABLE IF NOT EXISTS site_webhooks (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    site_id VARCHAR(36) NOT NULL,
    label VARCHAR(255) NOT NULL,
    url VARCHAR(2048) NOT NULL,
    headers_encrypted TEXT NOT NULL,
    created_by VARCHAR(36) NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS site_webhook_deliveries (
    id VARCHAR(36) PRIMARY KEY NOT NULL,
    webhook_id VARCHAR(36) NOT NULL,
    status VARCHAR(20) NOT NULL,
    status_code INTEGER,
    response_body TEXT,
    duration_ms INTEGER,
    triggered_by VARCHAR(36) NULL,
    triggered_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (webhook_id) REFERENCES site_webhooks(id) ON DELETE CASCADE,
    FOREIGN KEY (triggered_by) REFERENCES users(id) ON DELETE SET NULL,
    CONSTRAINT chk_delivery_status CHECK (status IN ('success', 'failed'))
);

CREATE INDEX idx_site_webhooks_site_id ON site_webhooks(site_id);
CREATE INDEX idx_site_webhook_deliveries_webhook_id ON site_webhook_deliveries(webhook_id);
