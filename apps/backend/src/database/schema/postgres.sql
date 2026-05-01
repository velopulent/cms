CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS sites (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    storage_provider TEXT NOT NULL DEFAULT 'filesystem',
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS site_members (
    id TEXT PRIMARY KEY NOT NULL,
    site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('owner', 'admin', 'editor', 'viewer')),
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
    singleton_data TEXT,
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
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    published_at TIMESTAMP WITH TIME ZONE
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

CREATE TABLE IF NOT EXISTS access_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('instance', 'site')),
    site_id TEXT REFERENCES sites(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    token_prefix TEXT NOT NULL,
    token_hmac TEXT,
    scopes TEXT NOT NULL,
    created_by_user_id TEXT REFERENCES users(id),
    last_used_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,
    revoked_at TIMESTAMP WITH TIME ZONE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_access_tokens_hash ON access_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_access_tokens_prefix ON access_tokens(token_prefix);
CREATE INDEX IF NOT EXISTS idx_access_tokens_kind ON access_tokens(kind);
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
    revision_number INTEGER NOT NULL,
    data JSONB NOT NULL,
    created_by TEXT REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    change_summary TEXT,
    UNIQUE(entry_id, revision_number)
);

CREATE INDEX IF NOT EXISTS idx_entry_revisions_entry_id ON entry_revisions(entry_id);
CREATE INDEX IF NOT EXISTS idx_entry_revisions_entry_number ON entry_revisions(entry_id, revision_number);
CREATE INDEX IF NOT EXISTS idx_entry_revisions_created_at ON entry_revisions(entry_id, created_at DESC);
