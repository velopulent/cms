use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, watch};

use crate::database::pool::DbPool;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InstanceSettings {
    pub version: u32,
    pub general: GeneralSettings,
    pub security: SecuritySettings,
    pub storage: StorageSettings,
    pub backups: BackupSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GeneralSettings {
    pub public_url: Option<String>,
    pub public_registration: bool,
    pub session_lifetime_hours: u64,
    pub upload_limit_mb: usize,
    pub mcp_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SecuritySettings {
    pub secure_cookies: bool,
    pub allowed_origins: Vec<String>,
    pub trusted_proxy_headers: bool,
    pub private_webhook_targets: bool,
    pub mcp_allowed_hosts: Vec<String>,
    pub mcp_allowed_origins: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StorageSettings {
    pub provider: String,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub public_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackupSettings {
    pub enabled: bool,
    pub destination: String,
    pub retention: u32,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialPair {
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedCredentials {
    pub storage: Option<CredentialPair>,
    pub backups: Option<CredentialPair>,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            general: GeneralSettings {
                public_url: None,
                public_registration: false,
                session_lifetime_hours: 24,
                upload_limit_mb: 50,
                mcp_enabled: true,
            },
            security: SecuritySettings {
                secure_cookies: false,
                allowed_origins: Vec::new(),
                trusted_proxy_headers: false,
                private_webhook_targets: false,
                mcp_allowed_hosts: vec!["localhost".into(), "127.0.0.1".into()],
                mcp_allowed_origins: Vec::new(),
            },
            storage: StorageSettings {
                provider: "filesystem".into(),
                bucket: None,
                region: Some("us-east-1".into()),
                endpoint: None,
                public_url: None,
            },
            backups: BackupSettings {
                enabled: true,
                destination: "filesystem".into(),
                retention: 7,
                bucket: None,
                region: Some("us-east-1".into()),
                endpoint: None,
            },
        }
    }
}

#[derive(Clone)]
pub struct SettingsService {
    pool: DbPool,
    key: Arc<[u8; 32]>,
    snapshot: watch::Sender<Arc<InstanceSettings>>,
    credentials: Arc<RwLock<EncryptedCredentials>>,
}

impl SettingsService {
    pub async fn load(pool: DbPool, master_key: &str) -> Result<Self, String> {
        let key = decode_key(&crate::secrets::derive_key_hex(
            master_key,
            "instance-settings-encryption",
        ))?;
        let row = load_row(&pool).await.map_err(|error| error.to_string())?;
        let (settings, credentials) = match row {
            Some((version, settings_json, encrypted)) => {
                if version != SETTINGS_VERSION as i64 {
                    return Err(format!("unsupported instance settings version {version}"));
                }
                let settings = serde_json::from_str(&settings_json).map_err(|error| error.to_string())?;
                let credentials = match encrypted.as_deref() {
                    Some(value) => decrypt_credentials(&key, value).unwrap_or_else(|error| {
                        tracing::error!(
                            "Encrypted integration credentials are unusable; integrations disabled: {error}"
                        );
                        EncryptedCredentials::default()
                    }),
                    None => EncryptedCredentials::default(),
                };
                (settings, credentials)
            }
            None => {
                let settings = InstanceSettings::default();
                persist(&pool, &settings, None)
                    .await
                    .map_err(|error| error.to_string())?;
                (settings, EncryptedCredentials::default())
            }
        };
        let (snapshot, _) = watch::channel(Arc::new(settings));
        Ok(Self {
            pool,
            key: Arc::new(key),
            snapshot,
            credentials: Arc::new(RwLock::new(credentials)),
        })
    }

    pub fn current(&self) -> Arc<InstanceSettings> {
        self.snapshot.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<InstanceSettings>> {
        self.snapshot.subscribe()
    }

    pub async fn credentials(&self) -> EncryptedCredentials {
        self.credentials.read().await.clone()
    }

    pub async fn publish(&self, settings: InstanceSettings, credentials: EncryptedCredentials) -> Result<(), String> {
        validate(&settings)?;
        let encrypted = if credentials.storage.is_some() || credentials.backups.is_some() {
            Some(encrypt_credentials(&self.key, &credentials)?)
        } else {
            None
        };
        persist(&self.pool, &settings, encrypted.as_deref())
            .await
            .map_err(|error| error.to_string())?;
        *self.credentials.write().await = credentials;
        self.snapshot.send_replace(Arc::new(settings));
        Ok(())
    }

    pub async fn reload(&self) -> Result<(), String> {
        let Some((version, settings_json, encrypted)) =
            load_row(&self.pool).await.map_err(|error| error.to_string())?
        else {
            return Err("instance settings row is missing after restore".into());
        };
        if version != SETTINGS_VERSION as i64 {
            return Err(format!("unsupported instance settings version {version}"));
        }
        let settings: InstanceSettings = serde_json::from_str(&settings_json).map_err(|error| error.to_string())?;
        validate(&settings)?;
        let credentials = match encrypted.as_deref() {
            Some(value) => decrypt_credentials(&self.key, value).unwrap_or_default(),
            None => EncryptedCredentials::default(),
        };
        *self.credentials.write().await = credentials;
        self.snapshot.send_replace(Arc::new(settings));
        Ok(())
    }

    pub async fn apply_to_config(&self, config: &mut crate::config::Config) {
        let settings = self.current();
        let credentials = self.credentials().await;
        config.public_url = settings.general.public_url.clone();
        config.public_registration_enabled = settings.general.public_registration;
        config.session_lifetime_hours = settings.general.session_lifetime_hours as i64;
        config.max_upload_size_bytes = settings.general.upload_limit_mb * 1024 * 1024;
        config.mcp_enabled = settings.general.mcp_enabled;
        config.cookie_secure = settings.security.secure_cookies;
        config.allowed_origins = settings.security.allowed_origins.clone();
        config.trust_proxy_headers = settings.security.trusted_proxy_headers;
        config.webhook_allow_private_targets = settings.security.private_webhook_targets;
        config.mcp_allowed_hosts = settings.security.mcp_allowed_hosts.clone();
        config.mcp_allowed_origins = settings.security.mcp_allowed_origins.clone();

        config.s3_bucket = settings.storage.bucket.clone();
        config.s3_region = settings.storage.region.clone();
        config.s3_endpoint = settings.storage.endpoint.clone();
        config.s3_public_url = settings.storage.public_url.clone();
        config.s3_access_key_id = credentials.storage.as_ref().map(|value| value.access_key_id.clone());
        config.s3_secret_access_key = credentials
            .storage
            .as_ref()
            .map(|value| value.secret_access_key.clone());

        config.backup_enabled = settings.backups.enabled;
        config.backup_destination = settings.backups.destination.clone();
        config.backup_default_retention = settings.backups.retention as i64;
        config.backup_s3_bucket = settings.backups.bucket.clone();
        config.backup_s3_region = settings.backups.region.clone();
        config.backup_s3_endpoint = settings.backups.endpoint.clone();
        config.backup_s3_access_key_id = credentials.backups.as_ref().map(|value| value.access_key_id.clone());
        config.backup_s3_secret_access_key = credentials
            .backups
            .as_ref()
            .map(|value| value.secret_access_key.clone());
    }
}

pub fn validate(settings: &InstanceSettings) -> Result<(), String> {
    if settings.version != SETTINGS_VERSION {
        return Err("settings version is not supported".into());
    }
    if !(1..=168).contains(&settings.general.session_lifetime_hours) {
        return Err("session_lifetime_hours must be between 1 and 168".into());
    }
    if !(1..=1024).contains(&settings.general.upload_limit_mb) {
        return Err("upload_limit_mb must be between 1 and 1024".into());
    }
    if let Some(value) = settings.general.public_url.as_deref() {
        validate_http_url(value, "public_url")?;
    }
    for origin in settings
        .security
        .allowed_origins
        .iter()
        .chain(&settings.security.mcp_allowed_origins)
    {
        let parsed = validate_http_url(origin, "allowed origin")?;
        if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(format!(
                "allowed origin must not contain a path, query, or fragment: {origin}"
            ));
        }
    }
    if settings
        .security
        .mcp_allowed_hosts
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("MCP allowed hosts cannot contain empty values".into());
    }
    if !matches!(settings.storage.provider.as_str(), "filesystem" | "s3") {
        return Err("storage provider must be filesystem or s3".into());
    }
    if !matches!(settings.backups.destination.as_str(), "filesystem" | "s3") {
        return Err("backup destination must be filesystem or s3".into());
    }
    if settings.storage.provider == "s3" && settings.storage.bucket.as_deref().is_none_or(str::is_empty) {
        return Err("storage bucket is required for S3".into());
    }
    if settings.backups.destination == "s3" && settings.backups.bucket.as_deref().is_none_or(str::is_empty) {
        return Err("backup bucket is required for S3".into());
    }
    if settings.backups.retention == 0 || settings.backups.retention > 10_000 {
        return Err("backup retention must be between 1 and 10000".into());
    }
    Ok(())
}

fn validate_http_url<'a>(value: &'a str, name: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(value).map_err(|error| format!("{name} is invalid: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(format!("{name} must be an absolute HTTP(S) URL"));
    }
    Ok(parsed)
}

async fn load_row(pool: &DbPool) -> Result<Option<(i64, String, Option<String>)>, sqlx::Error> {
    match pool {
        DbPool::Postgres(pool) => {
            sqlx::query_as(
                "SELECT version::bigint, settings_json, credentials_encrypted FROM instance_settings WHERE id = 1",
            )
            .fetch_optional(pool)
            .await
        }
        DbPool::MySql(pool) => sqlx::query_as(
            "SELECT CAST(version AS SIGNED), settings_json, credentials_encrypted FROM instance_settings WHERE id = 1",
        )
        .fetch_optional(pool)
        .await,
        DbPool::Sqlite(pool) => {
            sqlx::query_as("SELECT version, settings_json, credentials_encrypted FROM instance_settings WHERE id = 1")
                .fetch_optional(pool)
                .await
        }
    }
}

async fn persist(pool: &DbPool, settings: &InstanceSettings, encrypted: Option<&str>) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string(settings).map_err(|error| sqlx::Error::Encode(error.into()))?;
    match pool {
        DbPool::Postgres(pool) => sqlx::query("INSERT INTO instance_settings (id, version, settings_json, credentials_encrypted) VALUES (1, $1, $2, $3) ON CONFLICT (id) DO UPDATE SET version = EXCLUDED.version, settings_json = EXCLUDED.settings_json, credentials_encrypted = EXCLUDED.credentials_encrypted, updated_at = NOW()")
            .bind(SETTINGS_VERSION as i32).bind(json).bind(encrypted).execute(pool).await.map(|_| ()),
        DbPool::MySql(pool) => sqlx::query("INSERT INTO instance_settings (id, version, settings_json, credentials_encrypted) VALUES (1, ?, ?, ?) ON DUPLICATE KEY UPDATE version = VALUES(version), settings_json = VALUES(settings_json), credentials_encrypted = VALUES(credentials_encrypted)")
            .bind(SETTINGS_VERSION).bind(json).bind(encrypted).execute(pool).await.map(|_| ()),
        DbPool::Sqlite(pool) => sqlx::query("INSERT INTO instance_settings (id, version, settings_json, credentials_encrypted) VALUES (1, ?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET version = excluded.version, settings_json = excluded.settings_json, credentials_encrypted = excluded.credentials_encrypted, updated_at = datetime('now')")
            .bind(SETTINGS_VERSION).bind(json).bind(encrypted).execute(pool).await.map(|_| ()),
    }
}

fn decode_key(value: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(value).map_err(|error| error.to_string())?;
    bytes.try_into().map_err(|_| "derived key must be 32 bytes".into())
}

fn encrypt_credentials(key: &[u8; 32], credentials: &EncryptedCredentials) -> Result<String, String> {
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let mut nonce = [0_u8; 12];
    rand::rng().fill_bytes(&mut nonce);
    let plain = serde_json::to_vec(credentials).map_err(|error| error.to_string())?;
    let nonce_value = Nonce::from(nonce);
    let cipher_text = cipher
        .encrypt(&nonce_value, plain.as_ref())
        .map_err(|_| "credential encryption failed")?;
    let mut envelope = nonce.to_vec();
    envelope.extend(cipher_text);
    Ok(format!("v1:{}", BASE64_STANDARD.encode(envelope)))
}

fn decrypt_credentials(key: &[u8; 32], envelope: &str) -> Result<EncryptedCredentials, String> {
    let encoded = envelope
        .strip_prefix("v1:")
        .ok_or("unsupported credential envelope version")?;
    let bytes = BASE64_STANDARD.decode(encoded).map_err(|error| error.to_string())?;
    if bytes.len() < 13 {
        return Err("credential envelope is truncated".into());
    }
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let nonce: [u8; 12] = bytes[..12].try_into().map_err(|_| "credential nonce is invalid")?;
    let nonce_value = Nonce::from(nonce);
    let plain = cipher
        .decrypt(&nonce_value, &bytes[12..])
        .map_err(|_| "credential decryption failed")?;
    serde_json::from_slice(&plain).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_limits_and_origins() {
        let mut settings = InstanceSettings::default();
        settings.general.upload_limit_mb = 0;
        assert!(validate(&settings).unwrap_err().contains("upload_limit_mb"));
        settings.general.upload_limit_mb = 50;
        settings.security.allowed_origins = vec!["https://example.com/path".into()];
        assert!(validate(&settings).unwrap_err().contains("must not contain a path"));
    }

    #[test]
    fn credential_envelope_is_versioned_and_authenticated() {
        let key = [7_u8; 32];
        let credentials = EncryptedCredentials {
            storage: Some(CredentialPair {
                access_key_id: "AKIA_TEST".into(),
                secret_access_key: "secret".into(),
            }),
            backups: None,
        };
        let encrypted = encrypt_credentials(&key, &credentials).unwrap();
        assert!(encrypted.starts_with("v1:"));
        assert!(!encrypted.contains("secret"));
        assert_eq!(
            decrypt_credentials(&key, &encrypted)
                .unwrap()
                .storage
                .unwrap()
                .access_key_id,
            "AKIA_TEST"
        );
        assert!(decrypt_credentials(&[8_u8; 32], &encrypted).is_err());
    }
}
