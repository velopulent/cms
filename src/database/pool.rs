use sqlx::{
    mysql::{MySqlPool, MySqlPoolOptions},
    postgres::{PgPool, PgPoolOptions},
    sqlite::{SqlitePool, SqlitePoolOptions},
    Error,
};
use std::str::FromStr;

use crate::database::backend::DatabaseBackend;

#[derive(Clone)]
pub enum DbPool {
    Postgres(PgPool),
    MySql(MySqlPool),
    Sqlite(SqlitePool),
}

impl DbPool {
    pub async fn from_url(url: &str) -> Result<Self, Error> {
        let backend = DatabaseBackend::from_url(url)
            .ok_or_else(|| Error::Configuration("Unknown database URL scheme".into()))?;

        match backend {
            DatabaseBackend::Postgres => {
                let pool = PgPoolOptions::new()
                    .connect(url)
                    .await?;
                Ok(DbPool::Postgres(pool))
            }
            DatabaseBackend::MySQL => {
                let pool = MySqlPoolOptions::new()
                    .connect(url)
                    .await?;
                Ok(DbPool::MySql(pool))
            }
            DatabaseBackend::SQLite => {
                let options = sqlx::sqlite::SqliteConnectOptions::from_str(url)
                    .map_err(|e| Error::Configuration(e.to_string().into()))?
                    .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
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

    pub async fn execute(&self, query: &str) -> Result<u64, Error> {
        match self {
            DbPool::Postgres(pool) => {
                let result = sqlx::query(query).execute(pool).await?;
                Ok(result.rows_affected())
            }
            DbPool::MySql(pool) => {
                let result = sqlx::query(query).execute(pool).await?;
                Ok(result.rows_affected())
            }
            DbPool::Sqlite(pool) => {
                let result = sqlx::query(query).execute(pool).await?;
                Ok(result.rows_affected())
            }
        }
    }
}


