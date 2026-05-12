use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{Json, http::StatusCode, response::IntoResponse};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use url;
use uuid::Uuid;

use crate::models::webhook::{SiteWebhook, WebhookDelivery};
use crate::repository::error::RepositoryError;
use crate::repository::traits::WebhookRepository;

const WEBHOOK_TIMEOUT_SECS: u64 = 30;
const MAX_RESPONSE_BODY_CHARS: usize = 1024;

fn sanitize_url_for_logging(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("unknown");
            let path = if parsed.path().is_empty() { "/" } else { parsed.path() };
            format!("{}://{}{}", scheme, host, path)
        }
        Err(_) => "[invalid URL]".to_string(),
    }
}

#[derive(Clone)]
pub struct WebhookService {
    webhook_repo: Arc<dyn WebhookRepository>,
    encryption_key: Arc<[u8; 32]>,
}

#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Not found")]
    NotFound,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Invalid label: {0}")]
    InvalidLabel(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Webhook delivery failed: {0}")]
    DeliveryFailed(String),
}

impl WebhookError {
    pub fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            WebhookError::NotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "Webhook not found"}))),
            WebhookError::InvalidUrl(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            WebhookError::InvalidLabel(msg) => (StatusCode::BAD_REQUEST, Json(json!({"error": msg}))),
            WebhookError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))),
            WebhookError::DeliveryFailed(_) => (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "Webhook delivery failed"})),
            ),
        };
        (status, body).into_response()
    }
}

impl From<RepositoryError> for WebhookError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::NotFound => WebhookError::NotFound,
            _ => WebhookError::DatabaseError(err.to_string()),
        }
    }
}

impl WebhookService {
    pub fn new(webhook_repo: Arc<dyn WebhookRepository>, hmac_secret: &str) -> Self {
        let key = derive_encryption_key(hmac_secret);
        Self {
            webhook_repo,
            encryption_key: Arc::new(key),
        }
    }

    pub async fn list_webhooks(&self, site_id: &str) -> Result<Vec<SiteWebhook>, WebhookError> {
        self.webhook_repo
            .list_for_site(site_id)
            .await
            .map_err(WebhookError::from)
    }

    pub async fn get_webhook(&self, id: &str, site_id: &str) -> Result<Option<SiteWebhook>, WebhookError> {
        self.webhook_repo
            .get_by_id(id, site_id)
            .await
            .map_err(WebhookError::from)
    }

    pub async fn create_webhook(
        &self,
        site_id: &str,
        label: &str,
        url: &str,
        headers: &HashMap<String, String>,
        created_by: &str,
    ) -> Result<SiteWebhook, WebhookError> {
        debug!(
            "Creating webhook: site_id={}, label={}, url_pattern={}, headers_count={}",
            site_id,
            label,
            sanitize_url_for_logging(url),
            headers.len()
        );

        let label = label.trim();
        if label.is_empty() {
            warn!("Webhook creation failed: label is empty");
            return Err(WebhookError::InvalidLabel("Label is required".into()));
        }

        if let Err(e) = validate_url(url) {
            warn!("Webhook creation failed: invalid url_pattern={}, error={}", sanitize_url_for_logging(url), e);
            return Err(WebhookError::InvalidUrl(format!("Invalid URL: {}", e)));
        }

        let headers_encrypted = encrypt_headers(headers, &self.encryption_key);
        debug!("Headers encrypted for webhook");
        let id = Uuid::now_v7().to_string();

        debug!("Creating webhook record in repository: id={}", id);
        match self
            .webhook_repo
            .create(&id, site_id, label, url, &headers_encrypted, created_by)
            .await
        {
            Ok(webhook) => {
                info!(
                    "Webhook created successfully: id={}, site_id={}, label={}",
                    id, site_id, label
                );
                Ok(webhook)
            }
            Err(e) => {
                error!("Failed to create webhook in repository: id={}, error={}", id, e);
                Err(WebhookError::from(e))
            }
        }
    }

    pub async fn update_webhook(
        &self,
        id: &str,
        _site_id: &str,
        label: Option<&str>,
        url: Option<&str>,
        headers: Option<&HashMap<String, String>>,
    ) -> Result<SiteWebhook, WebhookError> {
        let url_display = url.map(sanitize_url_for_logging);
        debug!(
            "Updating webhook: id={}, label={:?}, url_pattern={:?}, headers_provided={}",
            id,
            label,
            url_display,
            headers.is_some()
        );

        if let Some(url_val) = url {
            if let Err(e) = validate_url(url_val) {
                warn!(
                    "Webhook update failed: invalid url_pattern={}, error={}",
                    sanitize_url_for_logging(url_val),
                    e
                );
                return Err(WebhookError::InvalidUrl(format!("Invalid URL: {}", e)));
            }
        }
        if let Some(label_val) = label {
            if label_val.trim().is_empty() {
                warn!("Webhook update failed: label is empty");
                return Err(WebhookError::InvalidLabel("Label cannot be empty".into()));
            }
        }

        let headers_encrypted = headers.map(|h| {
            debug!("Encrypting headers for webhook update");
            encrypt_headers(h, &self.encryption_key)
        });

        debug!("Updating webhook in repository: id={}", id);
        match self
            .webhook_repo
            .update(id, label, url, headers_encrypted.as_deref())
            .await
        {
            Ok(webhook) => {
                info!("Webhook updated successfully: id={}", id);
                Ok(webhook)
            }
            Err(e) => {
                error!("Failed to update webhook in repository: id={}, error={}", id, e);
                Err(WebhookError::from(e))
            }
        }
    }

    pub async fn delete_webhook(&self, id: &str, site_id: &str) -> Result<u64, WebhookError> {
        info!("Deleting webhook: id={}, site_id={}", id, site_id);

        match self.webhook_repo.delete(id, site_id).await {
            Ok(deleted_count) => {
                info!(
                    "Webhook deleted successfully: id={}, deleted_count={}",
                    id, deleted_count
                );
                Ok(deleted_count)
            }
            Err(e) => {
                error!("Failed to delete webhook: id={}, site_id={}, error={}", id, site_id, e);
                Err(WebhookError::from(e))
            }
        }
    }

    pub async fn trigger_webhook(
        &self,
        id: &str,
        site_id: &str,
        triggered_by: &str,
    ) -> Result<WebhookDelivery, WebhookError> {
        info!(
            "Triggering webhook: id={}, site_id={}, triggered_by={}",
            id, site_id, triggered_by
        );

        let webhook = self
            .webhook_repo
            .get_by_id(id, site_id)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch webhook for triggering: id={}, site_id={}, error={}",
                    id, site_id, e
                );
                WebhookError::from(e)
            })?
            .ok_or(WebhookError::NotFound)?;

        debug!(
            "Fetched webhook: id={}, url_pattern={}",
            webhook.id,
            sanitize_url_for_logging(&webhook.url)
        );

        let headers = decrypt_headers(&webhook.headers_encrypted, &self.encryption_key);
        debug!("Decrypted headers for webhook: header_count={}", headers.len());

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(WEBHOOK_TIMEOUT_SECS))
            .build()
            .map_err(|e| {
                error!("Failed to create HTTP client for webhook: error={}", e);
                WebhookError::DeliveryFailed(e.to_string())
            })?;

        let mut request = client.post(&webhook.url);
        debug!(
            "Making HTTP request to webhook: url_pattern={}",
            sanitize_url_for_logging(&webhook.url)
        );

        for (key, value) in &headers {
            request = request.header(key.as_str(), value.as_str());
        }
        request = request.header("Content-Type", "application/json");

        let start = std::time::Instant::now();
        let response = request.send().await;
        let duration_ms = start.elapsed().as_millis() as i64;

        debug!(
            "HTTP request completed: status={:?}, duration={}ms",
            response.as_ref().ok().map(|r| r.status()),
            duration_ms
        );

        let delivery_id = Uuid::now_v7().to_string();

        match response {
            Ok(resp) => {
                let status_code = resp.status().as_u16() as i32;
                let body = resp.text().await.unwrap_or_default();
                let truncated_body = truncate_str(&body, MAX_RESPONSE_BODY_CHARS);
                let success = status_code >= 200 && status_code < 300;
                let status = if success { "success" } else { "failed" };

                info!(
                    "Webhook delivery completed: id={}, status_code={}, success={}, duration={}ms",
                    delivery_id, status_code, success, duration_ms
                );

                self.webhook_repo
                    .create_delivery(
                        &delivery_id,
                        id,
                        status,
                        Some(status_code),
                        Some(&truncated_body),
                        Some(duration_ms),
                        triggered_by,
                    )
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to create webhook delivery record: id={}, error={}",
                            delivery_id, e
                        );
                        WebhookError::from(e)
                    })
            }
            Err(e) => {
                error!(
                    "HTTP request failed for webhook: id={}, error={}, duration={}ms",
                    id,
                    e.to_string(),
                    start.elapsed().as_millis() as i64
                );

                self.webhook_repo
                    .create_delivery(
                        &delivery_id,
                        id,
                        "failed",
                        None,
                        Some(&e.to_string()),
                        Some(duration_ms),
                        triggered_by,
                    )
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to create webhook delivery record for failed request: id={}, error={}",
                            delivery_id, e
                        );
                        WebhookError::from(e)
                    })
            }
        }
    }

    pub async fn list_deliveries(
        &self,
        webhook_id: &str,
        site_id: &str,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<WebhookDelivery>, i64), WebhookError> {
        let _webhook = self
            .webhook_repo
            .get_by_id(webhook_id, site_id)
            .await?
            .ok_or(WebhookError::NotFound)?;

        self.webhook_repo
            .list_deliveries(webhook_id, page, per_page)
            .await
            .map_err(WebhookError::from)
    }

    pub fn decrypt_webhook_headers(&self, webhook: &SiteWebhook) -> HashMap<String, String> {
        decrypt_headers(&webhook.headers_encrypted, &self.encryption_key)
    }
}

fn validate_url(url: &str) -> Result<(), WebhookError> {
    if url.trim().is_empty() {
        return Err(WebhookError::InvalidUrl("URL is required".into()));
    }
    if let Ok(parsed) = url::Url::parse(url) {
        if parsed.scheme() != "https" && parsed.scheme() != "http" {
            return Err(WebhookError::InvalidUrl("URL must use http or https scheme".into()));
        }
        Ok(())
    } else {
        Err(WebhookError::InvalidUrl("Invalid URL format".into()))
    }
}

fn derive_encryption_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b"cms-webhook-encryption-key");
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

fn encrypt_headers(headers: &HashMap<String, String>, key: &[u8; 32]) -> String {
    let json = serde_json::to_string(headers).unwrap_or_default();
    let plaintext = json.as_bytes();
    let mut encrypted = Vec::with_capacity(plaintext.len());
    for (i, byte) in plaintext.iter().enumerate() {
        encrypted.push(byte ^ key[i % key.len()]);
    }
    BASE64_STANDARD.encode(encrypted)
}

fn decrypt_headers(encrypted: &str, key: &[u8; 32]) -> HashMap<String, String> {
    let bytes = match BASE64_STANDARD.decode(encrypted) {
        Ok(b) => b,
        Err(_) => return HashMap::new(),
    };
    let mut decrypted = Vec::with_capacity(bytes.len());
    for (i, byte) in bytes.iter().enumerate() {
        decrypted.push(byte ^ key[i % key.len()]);
    }
    match String::from_utf8(decrypted) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        s[..max_chars].to_string()
    }
}
