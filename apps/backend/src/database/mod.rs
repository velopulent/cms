pub mod backend;
pub mod pool;

use pool::DbPool;

static SQLITE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/sqlite");
static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/postgres");
static MYSQL_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/mysql");

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
