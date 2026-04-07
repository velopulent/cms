use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::models::user::User;
use crate::repository::error::RepositoryError;
use crate::repository::traits::UserRepository;

pub struct MysqlUserRepository {
    pool: MySqlPool,
}

impl MysqlUserRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for MysqlUserRepository {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, RepositoryError> {
        let result = sqlx::query_as::<_, User>(
            "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        let result = sqlx::query_as::<_, User>(
            "SELECT id, username, email, password_hash, created_at, updated_at FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn find_id_by_username(&self, username: &str) -> Result<Option<String>, RepositoryError> {
        let result: Option<(String,)> =
            sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.map(|(id,)| id))
    }

    async fn create(
        &self,
        id: &str,
        username: &str,
        email: &str,
        password_hash: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn exists(&self, username: &str) -> Result<bool, RepositoryError> {
        let result: Option<(String,)> =
            sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.is_some())
    }

    async fn get_role(&self, user_id: &str, site_id: &str) -> Result<Option<String>, RepositoryError> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT sm.role FROM site_members sm WHERE sm.user_id = ? AND sm.site_id = ?",
        )
        .bind(user_id)
        .bind(site_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(role,)| role))
    }
}
