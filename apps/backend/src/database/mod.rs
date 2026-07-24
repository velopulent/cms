pub mod backend;
pub mod pool;

use backend::DatabaseBackend;
use pool::DbPool;

static SQLITE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/sqlite");
static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/postgres");

/// The highest migration version known to this binary for a backend. Used to
/// stamp backups and to refuse restoring a backup taken on a newer schema.
pub fn latest_migration_version(backend: DatabaseBackend) -> i64 {
    let migrator = match backend {
        DatabaseBackend::Postgres => &POSTGRES_MIGRATOR,
        DatabaseBackend::SQLite => &SQLITE_MIGRATOR,
    };
    migrator.iter().map(|m| m.version).max().unwrap_or(0)
}

pub async fn init_db(database_url: &str) -> Result<DbPool, sqlx::Error> {
    // Only the database URL and pool sizing matter for connecting; everything
    // else falls back to defaults.
    let pool = DbPool::from_url_with_config(&crate::config::Config {
        database_url: database_url.to_string(),
        db_max_connections: 10,
        db_min_connections: 2,
        db_acquire_timeout_secs: 30,
        db_idle_timeout_secs: 600,
        ..Default::default()
    })
    .await?;

    pool.run_migrations()
        .await
        .map_err(|e| sqlx::Error::Configuration(e.into()))?;
    Ok(pool)
}

pub async fn init_db_with_config(config: &crate::config::Config) -> Result<DbPool, sqlx::Error> {
    let pool = DbPool::from_url_with_config(config).await?;
    pool.run_migrations()
        .await
        .map_err(|e| sqlx::Error::Configuration(e.into()))?;
    Ok(pool)
}

pub async fn connect_db_without_migrations(
    config: &crate::config::Config,
) -> Result<DbPool, Box<dyn std::error::Error>> {
    let pool = DbPool::from_existing_with_config(config).await.map_err(|error| {
        format!(
            "Unable to connect to the existing CMS database without creating or migrating it: {error}. \
             Run `vcms serve` once to initialize and migrate the database."
        )
    })?;

    pool.validate_migrations().await.map_err(|error| {
        format!(
            "CMS database schema is missing, outdated, or incompatible: {error}. \
             Run `vcms serve` once to apply migrations."
        )
    })?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::{connect_db_without_migrations, init_db_with_config};
    use crate::config::Config;
    use crate::database::pool::DbPool;

    fn sqlite_config(path: &std::path::Path) -> Config {
        Config {
            database_url: format!("sqlite://{}", path.to_string_lossy().replace('\\', "/")),
            db_max_connections: 2,
            db_min_connections: 1,
            db_acquire_timeout_secs: 5,
            db_idle_timeout_secs: 60,
            ..Config::default()
        }
    }

    #[tokio::test]
    async fn connect_without_migrations_does_not_create_sqlite_database() {
        let directory = tempfile::tempdir().expect("temp directory");
        let database_path = directory.path().join("missing.db");
        let config = sqlite_config(&database_path);

        let result = connect_db_without_migrations(&config).await;

        assert!(result.is_err());
        assert!(!database_path.exists(), "stdio connection must not create database");
    }

    #[tokio::test]
    async fn connect_without_migrations_accepts_current_schema() {
        let directory = tempfile::tempdir().expect("temp directory");
        let database_path = directory.path().join("current.db");
        let config = sqlite_config(&database_path);
        let pool = init_db_with_config(&config).await.expect("migrated database");
        drop(pool);

        connect_db_without_migrations(&config)
            .await
            .expect("current schema should be accepted");
    }

    #[tokio::test]
    async fn connect_without_migrations_rejects_pending_schema_without_applying_it() {
        let directory = tempfile::tempdir().expect("temp directory");
        let database_path = directory.path().join("pending.db");
        let config = sqlite_config(&database_path);
        let pool = init_db_with_config(&config).await.expect("migrated database");
        let DbPool::Sqlite(sqlite) = pool else {
            panic!("expected sqlite pool");
        };
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 20260611000000")
            .execute(&sqlite)
            .await
            .expect("remove latest migration record");
        drop(sqlite);

        let result = connect_db_without_migrations(&config).await;
        assert!(result.is_err(), "pending schema should be rejected");

        let pool = DbPool::from_existing_with_config(&config)
            .await
            .expect("database should still exist");
        let DbPool::Sqlite(sqlite) = pool else {
            panic!("expected sqlite pool");
        };
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 20260611000000")
            .fetch_one(&sqlite)
            .await
            .expect("read migration table");
        assert_eq!(count, 0, "validation must not apply pending migration");
    }
}
