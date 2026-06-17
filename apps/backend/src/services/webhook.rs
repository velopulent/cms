use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use axum::{Json, http::StatusCode, response::IntoResponse};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use rand::RngCore;
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
    allow_private_targets: bool,
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
    pub fn new(webhook_repo: Arc<dyn WebhookRepository>, hmac_secret: &str, allow_private_targets: bool) -> Self {
        let key = derive_encryption_key(hmac_secret);
        Self {
            webhook_repo,
            encryption_key: Arc::new(key),
            allow_private_targets,
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
        created_by: Option<&str>,
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

        if let Err(e) = validate_url(url, self.allow_private_targets) {
            warn!(
                "Webhook creation failed: invalid url_pattern={}, error={}",
                sanitize_url_for_logging(url),
                e
            );
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

        if let Some(url_val) = url
            && let Err(e) = validate_url(url_val, self.allow_private_targets)
        {
            warn!(
                "Webhook update failed: invalid url_pattern={}, error={}",
                sanitize_url_for_logging(url_val),
                e
            );
            return Err(WebhookError::InvalidUrl(format!("Invalid URL: {}", e)));
        }
        if let Some(label_val) = label
            && label_val.trim().is_empty()
        {
            warn!("Webhook update failed: label is empty");
            return Err(WebhookError::InvalidLabel("Label cannot be empty".into()));
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
        triggered_by: Option<&str>,
    ) -> Result<WebhookDelivery, WebhookError> {
        info!(
            "Triggering webhook: id={}, site_id={}, triggered_by={:?}",
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

        // Re-validate and resolve right before sending: the stored host may now
        // resolve to a private address (DNS rebinding). Pin the connection to a
        // vetted IP and refuse redirects so it can't be bounced internally.
        validate_url(&webhook.url, self.allow_private_targets)?;
        let parsed_url =
            url::Url::parse(&webhook.url).map_err(|_| WebhookError::InvalidUrl("Invalid URL format".into()))?;
        let safe_addr = resolve_safe_addr(&parsed_url, self.allow_private_targets).await?;
        let host = parsed_url.host_str().unwrap_or_default().to_string();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(WEBHOOK_TIMEOUT_SECS))
            .redirect(reqwest::redirect::Policy::none())
            .resolve(&host, safe_addr)
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
                let success = (200..300).contains(&status_code);
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

/// Structural SSRF validation done at create/update time (no DNS lookup).
/// Rejects non-http(s) schemes, embedded credentials, localhost aliases, and
/// IP literals that point at private/loopback/link-local ranges. DNS-name hosts
/// are re-checked against their resolved IPs at delivery time (see
/// [`resolve_safe_addr`]) to defend against DNS rebinding.
fn validate_url(url: &str, allow_private: bool) -> Result<(), WebhookError> {
    if url.trim().is_empty() {
        return Err(WebhookError::InvalidUrl("URL is required".into()));
    }
    let parsed = url::Url::parse(url).map_err(|_| WebhookError::InvalidUrl("Invalid URL format".into()))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(WebhookError::InvalidUrl("URL must use http or https scheme".into()));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(WebhookError::InvalidUrl("URL must not contain credentials".into()));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| WebhookError::InvalidUrl("URL must have a host".into()))?;

    // Escape hatch for trusted internal/on-prem endpoints (and tests).
    if allow_private {
        return Ok(());
    }

    let host_lower = host.to_ascii_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return Err(WebhookError::InvalidUrl("URL host is not allowed".into()));
    }
    // IP literal? validate the address directly.
    if let Ok(ip) = host_lower
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        && is_disallowed_ip(ip)
    {
        return Err(WebhookError::InvalidUrl(
            "URL host resolves to a disallowed address".into(),
        ));
    }
    Ok(())
}

/// Resolve a webhook URL's host and ensure every resolved address is publicly
/// routable. Returns one safe `SocketAddr` to pin the outgoing connection to,
/// closing the DNS-rebinding window between validation and connect.
async fn resolve_safe_addr(parsed: &url::Url, allow_private: bool) -> Result<std::net::SocketAddr, WebhookError> {
    let host = parsed
        .host_str()
        .ok_or_else(|| WebhookError::InvalidUrl("URL must have a host".into()))?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| WebhookError::InvalidUrl("URL has no known port".into()))?;

    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| WebhookError::DeliveryFailed(format!("DNS resolution failed: {e}")))?;

    let mut chosen = None;
    for addr in addrs {
        if !allow_private && is_disallowed_ip(addr.ip()) {
            return Err(WebhookError::InvalidUrl(
                "URL host resolves to a disallowed address".into(),
            ));
        }
        chosen.get_or_insert(addr);
    }
    chosen.ok_or_else(|| WebhookError::InvalidUrl("URL host did not resolve".into()))
}

/// True if an IP must never be the target of an outbound webhook (loopback,
/// private, link-local, CGNAT, unique-local, multicast, unspecified, etc.).
fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
                || o[0] == 0
                // CGNAT 100.64.0.0/10
                || (o[0] == 100 && (64..=127).contains(&o[1]))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_disallowed_ip(IpAddr::V4(v4));
            }
            let seg0 = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg0 & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (seg0 & 0xffc0) == 0xfe80 // link local fe80::/10
        }
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

/// Encrypt webhook headers with AES-256-GCM. Output is
/// `base64(nonce[12] || ciphertext||tag)`. A fresh random nonce is used per call.
fn encrypt_headers(headers: &HashMap<String, String>, key: &[u8; 32]) -> String {
    let json = serde_json::to_string(headers).unwrap_or_default();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    match cipher.encrypt(nonce, json.as_bytes()) {
        Ok(ciphertext) => {
            let mut out = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
            out.extend_from_slice(&nonce_bytes);
            out.extend_from_slice(&ciphertext);
            BASE64_STANDARD.encode(out)
        }
        Err(_) => String::new(),
    }
}

/// Decrypt AES-256-GCM headers produced by [`encrypt_headers`]. Any failure
/// (bad base64, wrong key, truncated data, or legacy/incompatible ciphertext)
/// yields an empty map rather than an error.
fn decrypt_headers(encrypted: &str, key: &[u8; 32]) -> HashMap<String, String> {
    let raw = match BASE64_STANDARD.decode(encrypted) {
        Ok(b) => b,
        Err(_) => return HashMap::new(),
    };
    if raw.len() < 12 {
        return HashMap::new();
    }
    let (nonce_bytes, ciphertext) = raw.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    match cipher.decrypt(nonce, ciphertext) {
        Ok(plaintext) => match String::from_utf8(plaintext) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => HashMap::new(),
        },
        Err(_) => HashMap::new(),
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    s.char_indices()
        .nth(max_chars)
        .map(|(i, _)| s[..i].to_string())
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::webhook::SiteWebhook;
    use crate::repository::error::RepositoryError;
    use crate::test_helpers::InMemoryWebhookRepository;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn test_webhook_repo() -> Arc<InMemoryWebhookRepository> {
        Arc::new(InMemoryWebhookRepository::new())
    }

    fn test_service(repo: Arc<InMemoryWebhookRepository>) -> WebhookService {
        WebhookService::new(repo, "test-secret-key-for-webhooks", false)
    }

    fn make_webhook(id: &str, site_id: &str) -> SiteWebhook {
        SiteWebhook {
            id: id.to_string(),
            site_id: site_id.to_string(),
            label: "Test Hook".to_string(),
            url: "https://example.com/hook".to_string(),
            headers_encrypted: String::new(),
            created_by: None,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    // ── Free function tests ──

    #[test]
    fn test_validate_url_valid() {
        assert!(validate_url("https://example.com/hook", false).is_ok());
        assert!(validate_url("https://example.com:8080/webhook", false).is_ok());
    }

    #[test]
    fn test_validate_url_blocks_ssrf() {
        // localhost aliases
        assert!(validate_url("http://localhost:8080/webhook", false).is_err());
        assert!(validate_url("http://foo.localhost/webhook", false).is_err());
        // private / loopback / link-local / cgnat IP literals
        assert!(validate_url("http://127.0.0.1/x", false).is_err());
        assert!(validate_url("http://10.0.0.5/x", false).is_err());
        assert!(validate_url("http://192.168.1.1/x", false).is_err());
        assert!(validate_url("http://172.16.0.1/x", false).is_err());
        assert!(validate_url("http://169.254.169.254/latest/meta-data", false).is_err());
        assert!(validate_url("http://100.64.0.1/x", false).is_err());
        assert!(validate_url("http://[::1]/x", false).is_err());
        // embedded credentials
        assert!(validate_url("https://user:pass@example.com/x", false).is_err());
    }

    #[test]
    fn test_is_disallowed_ip() {
        use std::net::IpAddr;
        assert!(is_disallowed_ip("127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_disallowed_ip("10.1.2.3".parse::<IpAddr>().unwrap()));
        assert!(is_disallowed_ip("169.254.169.254".parse::<IpAddr>().unwrap()));
        assert!(is_disallowed_ip("::1".parse::<IpAddr>().unwrap()));
        assert!(is_disallowed_ip("fd00::1".parse::<IpAddr>().unwrap()));
        assert!(!is_disallowed_ip("8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(!is_disallowed_ip("1.1.1.1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_validate_url_empty() {
        assert!(validate_url("", false).is_err());
        assert!(validate_url("   ", false).is_err());
    }

    #[test]
    fn test_validate_url_no_scheme() {
        assert!(validate_url("example.com/hook", false).is_err());
    }

    #[test]
    fn test_validate_url_ftp_scheme() {
        assert!(validate_url("ftp://example.com/hook", false).is_err());
    }

    #[test]
    fn test_sanitize_url_for_logging() {
        assert_eq!(
            sanitize_url_for_logging("https://example.com/path?q=1"),
            "https://example.com/path"
        );
        assert_eq!(sanitize_url_for_logging("not-a-url"), "[invalid URL]");
    }

    #[test]
    fn test_derive_encryption_key() {
        let key1 = derive_encryption_key("secret");
        let key2 = derive_encryption_key("secret");
        let key3 = derive_encryption_key("different");
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_encryption_key("test-key");
        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "value1".to_string());
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());

        let encrypted = encrypt_headers(&headers, &key);
        let decrypted = decrypt_headers(&encrypted, &key);
        assert_eq!(headers, decrypted);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let key = derive_encryption_key("test-key");
        let result = decrypt_headers("not-valid-base64!@#", &key);
        assert!(result.is_empty());
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let key1 = derive_encryption_key("secret1");
        let key2 = derive_encryption_key("secret2");
        let mut headers = HashMap::new();
        headers.insert("X-Header".to_string(), "secret-value".to_string());

        let encrypted = encrypt_headers(&headers, &key1);
        let decrypted = decrypt_headers(&encrypted, &key2);
        assert!(decrypted.is_empty() || decrypted != headers);
    }

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        assert_eq!(truncate_str("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_str_empty() {
        assert_eq!(truncate_str("", 5), "");
    }

    #[test]
    fn test_truncate_str_multibyte_boundary() {
        // 'é' is 2 bytes in UTF-8; old byte-slice would panic here.
        assert_eq!(truncate_str("éééé", 3), "ééé");
    }

    #[test]
    fn test_truncate_str_multibyte_exact() {
        assert_eq!(truncate_str("日本語", 3), "日本語");
    }

    #[test]
    fn test_truncate_str_multibyte_short() {
        assert_eq!(truncate_str("日本語", 2), "日本");
    }

    #[test]
    fn test_truncate_str_emoji() {
        // Emoji can be 4 bytes
        assert_eq!(truncate_str("🦀🦞🐙", 2), "🦀🦞");
    }

    // ── WebhookError tests ──

    #[test]
    fn test_webhook_error_status_codes() {
        assert_eq!(WebhookError::NotFound.into_response().status(), StatusCode::NOT_FOUND);
        assert_eq!(
            WebhookError::InvalidUrl("bad".into()).into_response().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            WebhookError::InvalidLabel("bad".into()).into_response().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            WebhookError::DatabaseError("bad".into()).into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            WebhookError::DeliveryFailed("bad".into()).into_response().status(),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn test_webhook_error_from_repository_error() {
        let err: WebhookError = RepositoryError::NotFound.into();
        assert!(matches!(err, WebhookError::NotFound));

        let err: WebhookError = RepositoryError::Database("bad".into()).into();
        assert!(matches!(err, WebhookError::DatabaseError(_)));
    }

    // ── Service method tests ──

    #[tokio::test]
    async fn test_list_webhooks() {
        let repo = test_webhook_repo();
        repo.add_webhook(make_webhook("w1", "site-1"));
        repo.add_webhook(make_webhook("w2", "site-1"));
        repo.add_webhook(make_webhook("w3", "site-2"));
        let service = test_service(repo);

        let result = service.list_webhooks("site-1").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_webhooks_empty() {
        let service = test_service(test_webhook_repo());
        let result = service.list_webhooks("site-1").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_webhook_found() {
        let repo = test_webhook_repo();
        repo.add_webhook(make_webhook("w1", "site-1"));
        let service = test_service(repo);

        let result = service.get_webhook("w1", "site-1").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_get_webhook_not_found() {
        let service = test_service(test_webhook_repo());
        let result = service.get_webhook("nonexistent", "site-1").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_create_webhook_success() {
        let service = test_service(test_webhook_repo());
        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "value".to_string());

        let result = service
            .create_webhook(
                "site-1",
                "My Hook",
                "https://example.com/hook",
                &headers,
                Some("user-1"),
            )
            .await;
        assert!(result.is_ok());
        let webhook = result.unwrap();
        assert_eq!(webhook.label, "My Hook");
        assert_eq!(webhook.url, "https://example.com/hook");
        assert_eq!(webhook.site_id, "site-1");
    }

    #[tokio::test]
    async fn test_create_webhook_empty_label() {
        let service = test_service(test_webhook_repo());
        let result = service
            .create_webhook("site-1", "  ", "https://example.com/hook", &HashMap::new(), None)
            .await;
        assert!(matches!(result, Err(WebhookError::InvalidLabel(_))));
    }

    #[tokio::test]
    async fn test_create_webhook_invalid_url() {
        let service = test_service(test_webhook_repo());
        let result = service
            .create_webhook("site-1", "Hook", "not-a-url", &HashMap::new(), None)
            .await;
        assert!(matches!(result, Err(WebhookError::InvalidUrl(_))));
    }

    #[tokio::test]
    async fn test_create_webhook_ftp_url() {
        let service = test_service(test_webhook_repo());
        let result = service
            .create_webhook("site-1", "Hook", "ftp://example.com/file", &HashMap::new(), None)
            .await;
        assert!(matches!(result, Err(WebhookError::InvalidUrl(_))));
    }

    #[tokio::test]
    async fn test_update_webhook_success() {
        let repo = test_webhook_repo();
        repo.add_webhook(make_webhook("w1", "site-1"));
        let service = test_service(repo);

        let result = service
            .update_webhook("w1", "site-1", Some("New Label"), Some("https://new.com/hook"), None)
            .await;
        assert!(result.is_ok());
        let webhook = result.unwrap();
        assert_eq!(webhook.label, "New Label");
        assert_eq!(webhook.url, "https://new.com/hook");
    }

    #[tokio::test]
    async fn test_update_webhook_not_found() {
        let service = test_service(test_webhook_repo());
        let result = service
            .update_webhook("nonexistent", "site-1", Some("Label"), None, None)
            .await;
        assert!(matches!(result, Err(WebhookError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_webhook_empty_label() {
        let repo = test_webhook_repo();
        repo.add_webhook(make_webhook("w1", "site-1"));
        let service = test_service(repo);

        let result = service.update_webhook("w1", "site-1", Some("  "), None, None).await;
        assert!(matches!(result, Err(WebhookError::InvalidLabel(_))));
    }

    #[tokio::test]
    async fn test_update_webhook_invalid_url() {
        let repo = test_webhook_repo();
        repo.add_webhook(make_webhook("w1", "site-1"));
        let service = test_service(repo);

        let result = service
            .update_webhook("w1", "site-1", None, Some("not-a-url"), None)
            .await;
        assert!(matches!(result, Err(WebhookError::InvalidUrl(_))));
    }

    #[tokio::test]
    async fn test_delete_webhook_success() {
        let repo = test_webhook_repo();
        repo.add_webhook(make_webhook("w1", "site-1"));
        let service = test_service(repo);

        let result = service.delete_webhook("w1", "site-1").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_delete_webhook_not_found() {
        let service = test_service(test_webhook_repo());
        let result = service.delete_webhook("nonexistent", "site-1").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_list_deliveries() {
        let repo = test_webhook_repo();
        repo.add_webhook(make_webhook("w1", "site-1"));
        let service = test_service(repo);

        let result = service.list_deliveries("w1", "site-1", 1, 10).await;
        assert!(result.is_ok());
        let (items, total) = result.unwrap();
        assert!(items.is_empty());
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_list_deliveries_webhook_not_found() {
        let service = test_service(test_webhook_repo());
        let result = service.list_deliveries("nonexistent", "site-1", 1, 10).await;
        assert!(matches!(result, Err(WebhookError::NotFound)));
    }

    #[tokio::test]
    async fn test_decrypt_webhook_headers() {
        let repo = test_webhook_repo();
        let service = test_service(repo);

        let mut headers = HashMap::new();
        headers.insert("X-Auth".to_string(), "token123".to_string());
        let encrypted = encrypt_headers(&headers, &service.encryption_key);

        let webhook = SiteWebhook {
            id: "w1".to_string(),
            site_id: "site-1".to_string(),
            label: "Hook".to_string(),
            url: "https://example.com".to_string(),
            headers_encrypted: encrypted,
            created_by: None,
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        };

        let decrypted = service.decrypt_webhook_headers(&webhook);
        assert_eq!(decrypted, headers);
    }
}
