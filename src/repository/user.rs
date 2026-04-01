use sqlx::SqlitePool;

use crate::models::user::User;

pub async fn find_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_id_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<String>, sqlx::Error> {
    let result: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(pool)
            .await?;

    Ok(result.map(|(id,)| id))
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    username: &str,
    email: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn exists(pool: &SqlitePool, username: &str) -> Result<bool, sqlx::Error> {
    let result: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(pool)
            .await?;

    Ok(result.is_some())
}

pub async fn get_role(
    pool: &SqlitePool,
    user_id: &str,
    site_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT sm.role FROM site_members sm WHERE sm.user_id = ? AND sm.site_id = ?",
    )
    .bind(user_id)
    .bind(site_id)
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|(role,)| role))
}
