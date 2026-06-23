use async_trait::async_trait;
use sqlx::PgPool;
use tracing::debug;

use crate::models::user::User;
use crate::repository::error::RepositoryError;
use crate::repository::traits::UserRepository;

pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        debug!("Finding user by email");
        let result = sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash, instance_role, must_change_password, created_at::text as created_at, updated_at::text as updated_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        debug!("User lookup performed");
        Ok(result)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<User>, RepositoryError> {
        debug!("Finding user by id");
        let result = sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash, instance_role, must_change_password, created_at::text as created_at, updated_at::text as updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        debug!("User lookup performed");
        Ok(result)
    }

    async fn list(&self) -> Result<Vec<User>, RepositoryError> {
        Ok(sqlx::query_as::<_, User>(
            "SELECT id, name, email, password_hash, instance_role, must_change_password, created_at::text as created_at, updated_at::text as updated_at FROM users ORDER BY created_at, name"
        ).fetch_all(&self.pool).await?)
    }

    async fn find_id_by_name(&self, name: &str) -> Result<Option<String>, RepositoryError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result.map(|(id,)| id))
    }

    async fn create(&self, id: &str, name: &str, email: &str, password_hash: &str) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO users (id, name, email, password_hash) VALUES ($1, $2, $3, $4)")
            .bind(id)
            .bind(name)
            .bind(email)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn exists(&self, name: &str) -> Result<bool, RepositoryError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result.is_some())
    }

    async fn get_role(&self, user_id: &str, site_id: &str) -> Result<Option<String>, RepositoryError> {
        let result: Option<(String,)> =
            sqlx::query_as("SELECT sm.role FROM site_members sm WHERE sm.user_id = $1 AND sm.site_id = $2")
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
        Ok(sqlx::query("UPDATE users SET instance_role = $1 WHERE id = $2")
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
            "UPDATE users SET password_hash = $1, must_change_password = $2, updated_at = NOW() WHERE id = $3",
        )
        .bind(password_hash)
        .bind(must_change)
        .bind(user_id)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    async fn update_profile(&self, user_id: &str, name: &str, email: &str) -> Result<u64, RepositoryError> {
        Ok(
            sqlx::query("UPDATE users SET name = $1, email = $2, updated_at = NOW() WHERE id = $3")
                .bind(name)
                .bind(email)
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    async fn delete(&self, user_id: &str) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }
}
