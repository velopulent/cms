pub mod backend;
pub mod pool;

use pool::DbPool;

static SQLITE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/sqlite");
static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/postgres");
static MYSQL_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/mysql");

pub async fn init_db(database_url: &str) -> Result<DbPool, sqlx::Error> {
    let pool = DbPool::from_url_with_config(&crate::config::Config {
        database_url: database_url.to_string(),
        jwt_secret: String::new(),
        bind_address: String::new(),
        grpc_bind_address: String::new(),
        storage_fs_path: None,
        s3_access_key_id: None,
        s3_secret_access_key: None,
        s3_bucket: None,
        s3_region: None,
        s3_endpoint: None,
        s3_public_url: None,
        max_upload_size_bytes: 0,
        cookie_secure: false,
        db_max_connections: 10,
        db_min_connections: 2,
        db_acquire_timeout_secs: 30,
        db_idle_timeout_secs: 600,
        rate_limit_max_requests: 100,
        rate_limit_window_secs: 60,
        hmac_secret: String::new(),
        mcp_enabled: true,
        mcp_allowed_hosts: vec![],
        mcp_allowed_origins: vec![],
        public_url: None,
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
