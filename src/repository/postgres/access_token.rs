use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::access_token::{AccessToken, AccessTokenKind};
use crate::repository::error::RepositoryError;
use crate::repository::traits::{AccessTokenLookupRow, AccessTokenRepository};

pub struct PostgresAccessTokenRepository {
    pool: PgPool,
}

impl PostgresAccessTokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccessTokenRepository for PostgresAccessTokenRepository {
    async fn list(
        &self,
        kind: AccessTokenKind,
        site_id: Option<&str>,
    ) -> Result<Vec<AccessToken>, RepositoryError> {
        let rows = match site_id {
            Some(site_id) => {
                sqlx::query_as::<_, AccessToken>(
                    "SELECT id, kind, site_id, name, token_prefix, scopes, created_by_user_id, last_used_at::text as last_used_at, created_at::text as created_at, expires_at::text as expires_at, revoked_at::text as revoked_at
                     FROM access_tokens WHERE kind = $1 AND site_id = $2 ORDER BY created_at DESC",
                )
                .bind(kind.as_str())
                .bind(site_id)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, AccessToken>(
                    "SELECT id, kind, site_id, name, token_prefix, scopes, created_by_user_id, last_used_at::text as last_used_at, created_at::text as created_at, expires_at::text as expires_at, revoked_at::text as revoked_at
                     FROM access_tokens WHERE kind = $1 AND site_id IS NULL ORDER BY created_at DESC",
                )
                .bind(kind.as_str())
                .fetch_all(&self.pool)
                .await?
            }
        };

        Ok(rows)
    }

    async fn create(
        &self,
        id: &str,
        kind: AccessTokenKind,
        site_id: Option<&str>,
        name: &str,
        token_hash: &str,
        token_prefix: &str,
        token_hmac: &str,
        scopes: &str,
        created_by_user_id: Option<&str>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO access_tokens
             (id, kind, site_id, name, token_hash, token_prefix, token_hmac, scopes, created_by_user_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(kind.as_str())
        .bind(site_id)
        .bind(name)
        .bind(token_hash)
        .bind(token_prefix)
        .bind(token_hmac)
        .bind(scopes)
        .bind(created_by_user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(
        &self,
        id: &str,
        kind: AccessTokenKind,
        site_id: Option<&str>,
    ) -> Result<u64, RepositoryError> {
        let result = match site_id {
            Some(site_id) => {
                sqlx::query("DELETE FROM access_tokens WHERE id = $1 AND kind = $2 AND site_id = $3")
                    .bind(id)
                    .bind(kind.as_str())
                    .bind(site_id)
                    .execute(&self.pool)
                    .await?
            }
            None => {
                sqlx::query("DELETE FROM access_tokens WHERE id = $1 AND kind = $2 AND site_id IS NULL")
                    .bind(id)
                    .bind(kind.as_str())
                    .execute(&self.pool)
                    .await?
            }
        };

        Ok(result.rows_affected())
    }

    async fn find_by_prefix(&self, prefix: &str) -> Result<Vec<AccessTokenLookupRow>, RepositoryError> {
        let rows = sqlx::query_as::<_, AccessTokenLookupRow>(
            "SELECT id, kind, site_id, token_hash, token_hmac, expires_at::text, revoked_at::text, scopes
             FROM access_tokens WHERE token_prefix = $1",
        )
        .bind(prefix)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn update_last_used(&self, id: &str) -> Result<(), RepositoryError> {
        let _ = sqlx::query("UPDATE access_tokens SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await;
        Ok(())
    }
}
