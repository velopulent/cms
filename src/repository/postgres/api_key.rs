use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::api_key::ApiKey;
use crate::repository::error::RepositoryError;
use crate::repository::traits::ApiKeyRepository;

pub struct PostgresApiKeyRepository {
    pool: PgPool,
}

impl PostgresApiKeyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ApiKeyRepository for PostgresApiKeyRepository {
    async fn list(&self, site_id: &str) -> Result<Vec<ApiKey>, RepositoryError> {
        let result = sqlx::query_as::<_, ApiKey>(
            "SELECT id, site_id, name, key_prefix, permissions, last_used_at::text as last_used_at, created_at::text as created_at, expires_at::text as expires_at
             FROM api_keys WHERE site_id = $1 ORDER BY created_at DESC",
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        name: &str,
        key_hash: &str,
        key_prefix: &str,
        key_hmac: &str,
        permissions: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO api_keys (id, site_id, name, key_hash, key_prefix, key_hmac, permissions) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(site_id)
        .bind(name)
        .bind(key_hash)
        .bind(key_prefix)
        .bind(key_hmac)
        .bind(permissions)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM api_keys WHERE id = $1 AND site_id = $2")
            .bind(id)
            .bind(site_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn find_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, String, String, Option<String>, Option<String>, String)>, RepositoryError> {
        let result = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, String)>(
            "SELECT id, site_id, key_hash, key_hmac, expires_at::text, permissions FROM api_keys WHERE key_prefix = $1",
        )
        .bind(prefix)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn update_last_used(&self, id: &str) -> Result<(), RepositoryError> {
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await;
        Ok(())
    }
}
