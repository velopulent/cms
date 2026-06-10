use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{debug, error};

use crate::models::user::User;
use crate::repository::error::RepositoryError;
use crate::repository::traits::UserRepository;

pub struct SqliteUserRepository {
    pool: SqlitePool,
}

impl SqliteUserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, RepositoryError> {
        debug!("Finding user by username");
        let result = sqlx::query_as::<_, User>(
            "SELECT id, username, email, password_hash, instance_role, must_change_password, created_at, updated_at FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        debug!("User lookup performed");
        Ok(result)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        debug!("Finding user by id");
        let result = sqlx::query_as::<_, User>(
            "SELECT id, username, email, password_hash, instance_role, must_change_password, created_at, updated_at FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        debug!("User lookup performed");
        Ok(result)
    }

    async fn list(&self) -> Result<Vec<User>, RepositoryError> {
        Ok(sqlx::query_as::<_, User>(
            "SELECT id, username, email, password_hash, instance_role, must_change_password, created_at, updated_at FROM users ORDER BY created_at, username"
        ).fetch_all(&self.pool).await?)
    }

    async fn find_id_by_username(&self, username: &str) -> Result<Option<String>, RepositoryError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result.map(|(id,)| id))
    }

    async fn create(&self, id: &str, username: &str, email: &str, password_hash: &str) -> Result<(), RepositoryError> {
        debug!("Creating user");

        match sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(username)
            .bind(email)
            .bind(password_hash)
            .execute(&self.pool)
            .await
        {
            Ok(_) => {
                debug!("User created successfully: id={}", id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to create user: error occurred");
                Err(e.into())
            }
        }
    }

    async fn exists(&self, username: &str) -> Result<bool, RepositoryError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result.is_some())
    }

    async fn get_role(&self, user_id: &str, site_id: &str) -> Result<Option<String>, RepositoryError> {
        let result: Option<(String,)> =
            sqlx::query_as("SELECT sm.role FROM site_members sm WHERE sm.user_id = ? AND sm.site_id = ?")
                .bind(user_id)
                .bind(site_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.map(|(role,)| role))
    }

    async fn count(&self) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?)
    }

    async fn count_instance_owners(&self) -> Result<i64, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE instance_role = 'instance_owner'")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    async fn set_instance_role(&self, user_id: &str, role: Option<&str>) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("UPDATE users SET instance_role = ? WHERE id = ?")
            .bind(role)
            .bind(user_id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    async fn update_password(
        &self,
        user_id: &str,
        password_hash: &str,
        must_change: bool,
    ) -> Result<u64, RepositoryError> {
        Ok(sqlx::query(
            "UPDATE users SET password_hash = ?, must_change_password = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(password_hash)
        .bind(must_change)
        .bind(user_id)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }
}
