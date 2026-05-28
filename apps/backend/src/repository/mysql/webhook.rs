use async_trait::async_trait;
use sqlx::MySqlPool;

use crate::models::webhook::{SiteWebhook, WebhookDelivery};
use crate::repository::error::RepositoryError;
use crate::repository::traits::WebhookRepository;

pub struct MysqlWebhookRepository {
    pool: MySqlPool,
}

impl MysqlWebhookRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WebhookRepository for MysqlWebhookRepository {
    async fn list_for_site(&self, site_id: &str) -> Result<Vec<SiteWebhook>, RepositoryError> {
        let result = sqlx::query_as::<_, SiteWebhook>(
            "SELECT id, site_id, label, url, headers_encrypted, created_by, created_at, updated_at FROM site_webhooks WHERE site_id = ? ORDER BY created_at",
        )
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    async fn get_by_id(&self, id: &str, site_id: &str) -> Result<Option<SiteWebhook>, RepositoryError> {
        let result = sqlx::query_as::<_, SiteWebhook>(
            "SELECT id, site_id, label, url, headers_encrypted, created_by, created_at, updated_at FROM site_webhooks WHERE id = ? AND site_id = ?",
        )
        .bind(id)
        .bind(site_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn create(
        &self,
        id: &str,
        site_id: &str,
        label: &str,
        url: &str,
        headers_encrypted: &str,
        created_by: Option<&str>,
    ) -> Result<SiteWebhook, RepositoryError> {
        sqlx::query(
            "INSERT INTO site_webhooks (id, site_id, label, url, headers_encrypted, created_by) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(site_id)
        .bind(label)
        .bind(url)
        .bind(headers_encrypted)
        .bind(created_by)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id, site_id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn update(
        &self,
        id: &str,
        label: Option<&str>,
        url: Option<&str>,
        headers_encrypted: Option<&str>,
    ) -> Result<SiteWebhook, RepositoryError> {
        let existing = self.get_by_id_unscoped(id).await?.ok_or(RepositoryError::NotFound)?;

        let label = label.unwrap_or(&existing.label);
        let url = url.unwrap_or(&existing.url);
        let headers = headers_encrypted.unwrap_or(&existing.headers_encrypted);

        sqlx::query("UPDATE site_webhooks SET label = ?, url = ?, headers_encrypted = ? WHERE id = ?")
            .bind(label)
            .bind(url)
            .bind(headers)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.get_by_id_unscoped(id).await?.ok_or(RepositoryError::NotFound)
    }

    async fn delete(&self, id: &str, site_id: &str) -> Result<u64, RepositoryError> {
        let result = sqlx::query("DELETE FROM site_webhooks WHERE id = ? AND site_id = ?")
            .bind(id)
            .bind(site_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn create_delivery(
        &self,
        id: &str,
        webhook_id: &str,
        status: &str,
        status_code: Option<i32>,
        response_body: Option<&str>,
        duration_ms: Option<i64>,
        triggered_by: Option<&str>,
    ) -> Result<WebhookDelivery, RepositoryError> {
        sqlx::query(
            "INSERT INTO site_webhook_deliveries (id, webhook_id, status, status_code, response_body, duration_ms, triggered_by) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(webhook_id)
        .bind(status)
        .bind(status_code)
        .bind(response_body)
        .bind(duration_ms)
        .bind(triggered_by)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, WebhookDelivery>(
            "SELECT id, webhook_id, status, status_code, response_body, duration_ms, triggered_by, triggered_at FROM site_webhook_deliveries WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(RepositoryError::from)
    }

    async fn list_deliveries(
        &self,
        webhook_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<WebhookDelivery>, i64), RepositoryError> {
        let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM site_webhook_deliveries WHERE webhook_id = ?")
            .bind(webhook_id)
            .fetch_one(&self.pool)
            .await?;

        let offset = (page - 1) * per_page;
        let items = sqlx::query_as::<_, WebhookDelivery>(
            "SELECT id, webhook_id, status, status_code, response_body, duration_ms, triggered_by, triggered_at FROM site_webhook_deliveries WHERE webhook_id = ? ORDER BY triggered_at DESC LIMIT ? OFFSET ?",
        )
        .bind(webhook_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok((items, total.0))
    }
}

impl MysqlWebhookRepository {
    async fn get_by_id_unscoped(&self, id: &str) -> Result<Option<SiteWebhook>, RepositoryError> {
        sqlx::query_as::<_, SiteWebhook>(
            "SELECT id, site_id, label, url, headers_encrypted, created_by, created_at, updated_at FROM site_webhooks WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)
    }
}
