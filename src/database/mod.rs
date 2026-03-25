use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::str::FromStr;

pub async fn init_db() -> SqlitePool {
    let database_url = "sqlite:cms.db";

    let options = SqliteConnectOptions::from_str(database_url)
        .expect("Failed to parse database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to connect to database");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY NOT NULL,
            username TEXT UNIQUE NOT NULL,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS sites (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            created_by TEXT NOT NULL REFERENCES users(id),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS site_members (
            id TEXT PRIMARY KEY NOT NULL,
            site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            role TEXT NOT NULL CHECK(role IN ('owner', 'admin', 'editor', 'viewer')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(site_id, user_id)
        );

        CREATE TABLE IF NOT EXISTS schemas (
            id TEXT PRIMARY KEY NOT NULL,
            site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            definition JSON NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS content (
            id TEXT PRIMARY KEY NOT NULL,
            site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
            schema_id TEXT NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,
            data JSON NOT NULL,
            slug TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'published')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            published_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_site_members_user ON site_members(user_id);
        CREATE INDEX IF NOT EXISTS idx_site_members_site ON site_members(site_id);
        CREATE INDEX IF NOT EXISTS idx_schemas_site ON schemas(site_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_schemas_site_name ON schemas(site_id, name);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_schemas_site_slug ON schemas(site_id, slug);
        CREATE INDEX IF NOT EXISTS idx_content_site ON content(site_id);
        CREATE INDEX IF NOT EXISTS idx_content_slug ON content(slug);
        CREATE INDEX IF NOT EXISTS idx_content_schema ON content(schema_id);
        CREATE INDEX IF NOT EXISTS idx_content_status ON content(status);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_content_schema_slug ON content(schema_id, slug);
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to create tables");

    pool
}
