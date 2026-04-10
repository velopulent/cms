pub mod backend;
pub mod pool;

use backend::DatabaseBackend;
use pool::DbPool;

pub async fn init_db(database_url: &str) -> Result<DbPool, sqlx::Error> {
    let pool = DbPool::from_url_with_config(
        &crate::config::Config {
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
        },
    )
    .await?;

    let backend = pool.backend();
    run_schema(&pool, backend).await?;
    Ok(pool)
}

pub async fn init_db_with_config(config: &crate::config::Config) -> Result<DbPool, sqlx::Error> {
    let pool = DbPool::from_url_with_config(config).await?;
    let backend = pool.backend();
    run_schema(&pool, backend).await?;
    Ok(pool)
}

async fn run_schema(pool: &DbPool, backend: DatabaseBackend) -> Result<(), sqlx::Error> {
    let schema = match backend {
        DatabaseBackend::Postgres => include_str!("schema/postgres.sql"),
        DatabaseBackend::MySQL => include_str!("schema/mysql.sql"),
        DatabaseBackend::SQLite => include_str!("schema/sqlite.sql"),
    };

    for statement in schema.split(';').filter(|s| !s.trim().is_empty()) {
        pool.execute(statement).await?;
    }

    Ok(())
}
