use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::models::session::Session;
use crate::repository::error::RepositoryError;
use crate::repository::traits::SessionRepository;

pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn create(
        &self,
        id: &str,
        user_id: &str,
        token_hash: &str,
        csrf_token_hash: &str,
        expires_at: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token_hash, csrf_token_hash, expires_at) VALUES (?, ?, ?, ?, ?)",
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
        Ok(sqlx::query_as::<_, Session>(
            "SELECT id, user_id, token_hash, csrf_token_hash, created_at, expires_at, last_seen_at, revoked_at
             FROM sessions WHERE token_hash = ? AND revoked_at IS NULL AND expires_at > datetime('now')",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn touch(&self, id: &str) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE sessions SET last_seen_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn revoke(&self, id: &str, user_id: &str) -> Result<u64, RepositoryError> {
        Ok(sqlx::query(
            "UPDATE sessions SET revoked_at = datetime('now') WHERE id = ? AND user_id = ? AND revoked_at IS NULL",
        )
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    async fn revoke_all(&self, user_id: &str) -> Result<u64, RepositoryError> {
        Ok(
            sqlx::query("UPDATE sessions SET revoked_at = datetime('now') WHERE user_id = ? AND revoked_at IS NULL")
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    async fn list(&self, user_id: &str) -> Result<Vec<Session>, RepositoryError> {
        Ok(sqlx::query_as::<_, Session>(
            "SELECT id, user_id, token_hash, csrf_token_hash, created_at, expires_at, last_seen_at, revoked_at
             FROM sessions WHERE user_id = ? AND revoked_at IS NULL AND expires_at > datetime('now') ORDER BY created_at DESC"
        ).bind(user_id).fetch_all(&self.pool).await?)
    }
}
