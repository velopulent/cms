use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::models::access_token::AccessToken;
use crate::repository::error::RepositoryError;
use crate::repository::traits::{AccessTokenLookupRow, AccessTokenRepository, NewAccessToken};

pub struct SqliteAccessTokenRepository {
    pool: SqlitePool,
}

impl SqliteAccessTokenRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccessTokenRepository for SqliteAccessTokenRepository {
    async fn list(&self, site_id: &str) -> Result<Vec<AccessToken>, RepositoryError> {
        let rows = sqlx::query_as::<_, AccessToken>(
            "SELECT id, site_id, name, token_prefix, permission, created_by_user_id, last_used_at, created_at, expires_at, revoked_at
             FROM access_tokens WHERE site_id = ? ORDER BY created_at DESC",
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn create(&self, token: NewAccessToken<'_>) -> Result<(), RepositoryError> {
        let NewAccessToken {
            id,
            site_id,
            name,
            token_hash,
            token_prefix,
            token_hmac,
            permission,
            created_by_user_id,
        } = token;
        sqlx::query(
            "INSERT INTO access_tokens
             (id, site_id, name, token_hash, token_prefix, token_hmac, permission, created_by_user_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(site_id)
        .bind(name)
        .bind(token_hash)
        .bind(token_prefix)
        .bind(token_hmac)
        .bind(permission)
        .bind(created_by_user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM access_tokens WHERE id = ? AND site_id = ?")
            .bind(id)
            .bind(site_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn find_by_prefix(&self, prefix: &str) -> Result<Vec<AccessTokenLookupRow>, RepositoryError> {
        let rows = sqlx::query_as::<_, AccessTokenLookupRow>(
            "SELECT id, site_id, token_hash, token_hmac, expires_at, revoked_at, permission
             FROM access_tokens WHERE token_prefix = ?",
        )
        .bind(prefix)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn update_last_used(&self, id: &str) -> Result<(), RepositoryError> {
        let _ = sqlx::query("UPDATE access_tokens SET last_used_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await;
        Ok(())
    }
}
