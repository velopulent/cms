use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::str::FromStr;

pub async fn init_db(database_url: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(database_url)
        .expect("Failed to parse database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("Failed to connect to database");

    sqlx::query(include_str!("schema.sql"))
        .execute(&pool)
        .await
        .expect("Failed to create tables");

    pool
}
