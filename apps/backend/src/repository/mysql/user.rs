use async_trait::async_trait;
use sqlx::MySqlPool;
use tracing::debug;

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
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        debug!("Finding user by email");
        let result = sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash, instance_role, must_change_password, CAST(created_at AS CHAR) AS created_at, CAST(updated_at AS CHAR) AS updated_at FROM users WHERE email = ?",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        debug!("User lookup performed");
        Ok(result)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        let result = sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash, instance_role, must_change_password, CAST(created_at AS CHAR) AS created_at, CAST(updated_at AS CHAR) AS updated_at FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn list(&self) -> Result<Vec<User>, RepositoryError> {
        Ok(sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash, instance_role, must_change_password, CAST(created_at AS CHAR) AS created_at, CAST(updated_at AS CHAR) AS updated_at FROM users ORDER BY created_at, name"
        ).fetch_all(&self.pool).await?)
    }

    async fn create(&self, id: &str, name: &str, email: &str, password_hash: &str) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO users (id, name, email, password_hash) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(name)
            .bind(email)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn exists(&self, email: &str) -> Result<bool, RepositoryError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE email = ?")
            .bind(email)
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
        Ok(
            sqlx::query(
                "UPDATE users SET password_hash = ?, must_change_password = ?, updated_at = NOW() WHERE id = ?",
            )
            .bind(password_hash)
            .bind(must_change)
            .bind(user_id)
            .execute(&self.pool)
            .await?
            .rows_affected(),
        )
    }

    async fn update_profile(&self, user_id: &str, name: &str, email: &str) -> Result<u64, RepositoryError> {
        Ok(
            sqlx::query("UPDATE users SET name = ?, email = ?, updated_at = NOW() WHERE id = ?")
                .bind(name)
                .bind(email)
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    async fn delete(&self, user_id: &str) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }
}
