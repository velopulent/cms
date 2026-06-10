use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::models::session::Session;
use crate::repository::error::RepositoryError;
use crate::repository::traits::SessionRepository;

pub struct MysqlSessionRepository {
    pool: MySqlPool,
}
impl MysqlSessionRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for MysqlSessionRepository {
    async fn create(
        &self,
        id: &str,
        user_id: &str,
        token_hash: &str,
        csrf_token_hash: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token_hash, csrf_token_hash, expires_at)
             VALUES (?, ?, ?, ?, STR_TO_DATE(LEFT(?, 19), '%Y-%m-%dT%H:%i:%s'))",
        )
        .bind(id)
        .bind(user_id)
        .bind(token_hash)
        .bind(csrf_token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    async fn find_active_by_hash(&self, token_hash: &str) -> Result<Option<Session>, RepositoryError> {
        Ok(sqlx::query_as::<_, Session>("SELECT id, user_id, token_hash, csrf_token_hash, CAST(created_at AS CHAR) AS created_at, CAST(expires_at AS CHAR) AS expires_at, CAST(last_seen_at AS CHAR) AS last_seen_at, CAST(revoked_at AS CHAR) AS revoked_at FROM sessions WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > NOW()")
            .bind(token_hash).fetch_optional(&self.pool).await?)
    }
    async fn touch(&self, id: &str) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE sessions SET last_seen_at = NOW() WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    async fn revoke(&self, id: &str, user_id: &str) -> Result<u64, RepositoryError> {
        Ok(
            sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE id = ? AND user_id = ? AND revoked_at IS NULL")
                .bind(id)
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }
    async fn revoke_all(&self, user_id: &str) -> Result<u64, RepositoryError> {
        Ok(
            sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE user_id = ? AND revoked_at IS NULL")
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }
    async fn list(&self, user_id: &str) -> Result<Vec<Session>, RepositoryError> {
        Ok(sqlx::query_as::<_, Session>("SELECT id, user_id, token_hash, csrf_token_hash, CAST(created_at AS CHAR) AS created_at, CAST(expires_at AS CHAR) AS expires_at, CAST(last_seen_at AS CHAR) AS last_seen_at, CAST(revoked_at AS CHAR) AS revoked_at FROM sessions WHERE user_id = ? AND revoked_at IS NULL AND expires_at > NOW() ORDER BY created_at DESC").bind(user_id).fetch_all(&self.pool).await?)
    }
}
