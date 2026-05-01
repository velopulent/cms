use sqlx::{
    Error,
    mysql::{MySqlPool, MySqlPoolOptions},
    postgres::{PgPool, PgPoolOptions},
    sqlite::{SqlitePool, SqlitePoolOptions},
};
use std::str::FromStr;
use std::time::Duration;

use crate::config::Config;
use crate::database::backend::DatabaseBackend;
use crate::database::{SQLITE_MIGRATOR, POSTGRES_MIGRATOR, MYSQL_MIGRATOR};

#[derive(Clone)]
pub enum DbPool {
    Postgres(PgPool),
    MySql(MySqlPool),
    Sqlite(SqlitePool),
}

impl DbPool {
    pub async fn from_url_with_config(config: &Config) -> Result<Self, Error> {
        let backend = DatabaseBackend::from_url(&config.database_url)
            .ok_or_else(|| Error::Configuration("Unknown database URL scheme".into()))?;

        match backend {
            DatabaseBackend::Postgres => {
                let pool = PgPoolOptions::new()
                    .max_connections(config.db_max_connections)
                    .min_connections(config.db_min_connections)
                    .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs))
                    .idle_timeout(Duration::from_secs(config.db_idle_timeout_secs))
                    .connect(&config.database_url)
                    .await?;
                Ok(DbPool::Postgres(pool))
            }
            DatabaseBackend::MySQL => {
                let pool = MySqlPoolOptions::new()
                    .max_connections(config.db_max_connections)
                    .min_connections(config.db_min_connections)
                    .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs))
                    .idle_timeout(Duration::from_secs(config.db_idle_timeout_secs))
                    .connect(&config.database_url)
                    .await?;
                Ok(DbPool::MySql(pool))
            }
            DatabaseBackend::SQLite => {
                let options = sqlx::sqlite::SqliteConnectOptions::from_str(&config.database_url)
                    .map_err(|e| Error::Configuration(e.to_string().into()))?
                    .create_if_missing(true)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .busy_timeout(Duration::from_secs(30));
                let pool = SqlitePoolOptions::new()
                    .max_connections(config.db_max_connections)
                    .min_connections(config.db_min_connections)
                    .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs))
                    .idle_timeout(Duration::from_secs(config.db_idle_timeout_secs))
                    .connect_with(options)
                    .await?;
                Ok(DbPool::Sqlite(pool))
            }
        }
    }

    pub fn backend(&self) -> DatabaseBackend {
        match self {
            DbPool::Postgres(_) => DatabaseBackend::Postgres,
            DbPool::MySql(_) => DatabaseBackend::MySQL,
            DbPool::Sqlite(_) => DatabaseBackend::SQLite,
        }
    }

    pub async fn run_migrations(&self) -> Result<(), sqlx::migrate::MigrateError> {
        match self {
            DbPool::Postgres(pool) => POSTGRES_MIGRATOR.run(pool).await,
            DbPool::MySql(pool) => MYSQL_MIGRATOR.run(pool).await,
            DbPool::Sqlite(pool) => SQLITE_MIGRATOR.run(pool).await,
        }
    }
}
