use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::paths::RuntimePaths;
use crate::secrets;

pub const DEFAULT_LOG_LEVEL: &str = "cms=info,vcms=info";
pub const DEFAULT_MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapConfig {
    pub server: ServerConfig,
    pub log: LogConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub http_address: String,
    pub grpc_address: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    pub level: String,
    pub output: LogOutput,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    File,
    Stdout,
}

impl std::fmt::Display for LogOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::File => "file",
            Self::Stdout => "stdout",
        })
    }
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                http_address: "127.0.0.1:3000".to_owned(),
                grpc_address: "127.0.0.1:50051".to_owned(),
            },
            log: LogConfig {
                level: DEFAULT_LOG_LEVEL.to_owned(),
                output: LogOutput::File,
            },
        }
    }
}

/// Runtime compatibility carrier. Values no longer come from environment or CLI;
/// bootstrap values come from `config.toml`, secrets from `secrets.toml`, and the
/// remaining fields are fixed defaults until DB-backed instance settings load.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_address: String,
    pub grpc_bind_address: String,
    pub storage_fs_path: Option<String>,
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_public_url: Option<String>,
    pub backup_enabled: bool,
    pub backup_destination: String,
    pub backup_local_path: Option<String>,
    pub backup_zstd_level: i32,
    pub backup_default_retention: i64,
    pub backup_s3_access_key_id: Option<String>,
    pub backup_s3_secret_access_key: Option<String>,
    pub backup_s3_bucket: Option<String>,
    pub backup_s3_region: Option<String>,
    pub backup_s3_endpoint: Option<String>,
    pub backup_s3_public_url: Option<String>,
    pub backup_encryption_key: Option<String>,
    pub max_upload_size_bytes: usize,
    pub upload_token_expiry_secs: i64,
    pub cookie_secure: bool,
    pub session_lifetime_hours: i64,
    pub public_registration_enabled: bool,
    pub allowed_origins: Vec<String>,
    pub production: bool,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
    pub db_idle_timeout_secs: u64,
    pub rate_limit_max_requests: u32,
    pub rate_limit_window_secs: u64,
    pub bcrypt_cost: u32,
    pub trust_proxy_headers: bool,
    pub webhook_allow_private_targets: bool,
    pub token_index_key: String,
    pub session_auth_key: String,
    pub signed_upload_key: String,
    pub webhook_encryption_key: String,
    pub search_enabled: bool,
    pub search_index_path: Option<String>,
    pub mcp_enabled: bool,
    pub mcp_allowed_hosts: Vec<String>,
    pub mcp_allowed_origins: Vec<String>,
    pub public_url: Option<String>,
    pub log_level: String,
    pub log_output: String,
    pub log_dir: String,
}

impl Config {
    pub fn load(
        paths: &RuntimePaths,
        persisted: &secrets::PersistedSecrets,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let bootstrap = load_bootstrap(paths)?;
        validate_bootstrap(&bootstrap)?;

        Ok(Self {
            database_url: persisted.database_url.clone().unwrap_or_else(|| paths.database_url()),
            bind_address: bootstrap.server.http_address,
            grpc_bind_address: bootstrap.server.grpc_address,
            storage_fs_path: Some(paths.storage_dir().to_string_lossy().into_owned()),
            s3_access_key_id: None,
            s3_secret_access_key: None,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            s3_public_url: None,
            backup_enabled: true,
            backup_destination: "filesystem".to_owned(),
            backup_local_path: Some(paths.backups_dir().to_string_lossy().into_owned()),
            backup_zstd_level: 12,
            backup_default_retention: 7,
            backup_s3_access_key_id: None,
            backup_s3_secret_access_key: None,
            backup_s3_bucket: None,
            backup_s3_region: None,
            backup_s3_endpoint: None,
            backup_s3_public_url: None,
            backup_encryption_key: Some(persisted.backup_encryption_key.clone()),
            max_upload_size_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            upload_token_expiry_secs: crate::signed_upload::DEFAULT_UPLOAD_TOKEN_EXPIRY_SECS,
            cookie_secure: false,
            session_lifetime_hours: 24,
            public_registration_enabled: false,
            allowed_origins: Vec::new(),
            production: false,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            bcrypt_cost: bcrypt::DEFAULT_COST,
            trust_proxy_headers: false,
            webhook_allow_private_targets: false,
            token_index_key: secrets::derive_key_hex(&persisted.master_key, "access-token-index"),
            session_auth_key: secrets::derive_key_hex(&persisted.master_key, "dashboard-session-auth"),
            signed_upload_key: secrets::derive_key_hex(&persisted.master_key, "signed-upload-auth"),
            webhook_encryption_key: secrets::derive_key_hex(&persisted.master_key, "webhook-encryption"),
            search_enabled: true,
            search_index_path: Some(paths.search_dir().to_string_lossy().into_owned()),
            mcp_enabled: true,
            mcp_allowed_hosts: default_mcp_allowed_hosts(),
            mcp_allowed_origins: Vec::new(),
            public_url: None,
            log_level: bootstrap.log.level,
            log_output: bootstrap.log.output.to_string(),
            log_dir: paths.logs_dir().to_string_lossy().into_owned(),
        })
    }

    pub fn has_s3(&self) -> bool {
        self.s3_access_key_id.is_some() && self.s3_secret_access_key.is_some() && self.s3_bucket.is_some()
    }

    pub fn has_backup_s3(&self) -> bool {
        self.backup_s3_access_key_id.is_some()
            && self.backup_s3_secret_access_key.is_some()
            && self.backup_s3_bucket.is_some()
    }

    pub fn validate_security(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn redacted_toml(&self, paths: &RuntimePaths) -> String {
        let db_kind = if self.database_url.starts_with("postgres") {
            "postgres"
        } else if self.database_url.starts_with("mysql") {
            "mysql"
        } else {
            "sqlite"
        };
        format!(
            "mode = \"{}\"\nroot = \"{}\"\ndatabase = \"{} (configured)\"\n\n[secrets]\nmaster_key_present = true\nbackup_encryption_key_present = {}\ndatabase_url_present = {}\n\n[server]\nhttp_address = \"{}\"\ngrpc_address = \"{}\"\n\n[log]\nlevel = \"{}\"\noutput = \"{}\"\npath = \"{}\"\n",
            paths.mode(),
            paths.root().display(),
            db_kind,
            self.backup_encryption_key.is_some(),
            !self.database_url.starts_with("sqlite://"),
            self.bind_address,
            self.grpc_bind_address,
            self.log_level,
            self.log_output,
            paths.logs_dir().display(),
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        let paths = RuntimePaths::portable(".");
        let bootstrap = BootstrapConfig::default();
        let persisted = secrets::fresh(None);
        Self {
            database_url: paths.database_url(),
            bind_address: bootstrap.server.http_address,
            grpc_bind_address: bootstrap.server.grpc_address,
            storage_fs_path: Some(paths.storage_dir().to_string_lossy().into_owned()),
            s3_access_key_id: None,
            s3_secret_access_key: None,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            s3_public_url: None,
            backup_enabled: true,
            backup_destination: "filesystem".to_owned(),
            backup_local_path: Some(paths.backups_dir().to_string_lossy().into_owned()),
            backup_zstd_level: 12,
            backup_default_retention: 7,
            backup_s3_access_key_id: None,
            backup_s3_secret_access_key: None,
            backup_s3_bucket: None,
            backup_s3_region: None,
            backup_s3_endpoint: None,
            backup_s3_public_url: None,
            backup_encryption_key: Some(persisted.backup_encryption_key),
            max_upload_size_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            upload_token_expiry_secs: crate::signed_upload::DEFAULT_UPLOAD_TOKEN_EXPIRY_SECS,
            cookie_secure: false,
            session_lifetime_hours: 24,
            public_registration_enabled: false,
            allowed_origins: Vec::new(),
            production: false,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            bcrypt_cost: bcrypt::DEFAULT_COST,
            trust_proxy_headers: false,
            webhook_allow_private_targets: false,
            token_index_key: secrets::derive_key_hex(&persisted.master_key, "access-token-index"),
            session_auth_key: secrets::derive_key_hex(&persisted.master_key, "dashboard-session-auth"),
            signed_upload_key: secrets::derive_key_hex(&persisted.master_key, "signed-upload-auth"),
            webhook_encryption_key: secrets::derive_key_hex(&persisted.master_key, "webhook-encryption"),
            search_enabled: true,
            search_index_path: Some(paths.search_dir().to_string_lossy().into_owned()),
            mcp_enabled: true,
            mcp_allowed_hosts: default_mcp_allowed_hosts(),
            mcp_allowed_origins: Vec::new(),
            public_url: None,
            log_level: bootstrap.log.level,
            log_output: bootstrap.log.output.to_string(),
            log_dir: paths.logs_dir().to_string_lossy().into_owned(),
        }
    }
}

pub fn ensure_bootstrap(paths: &RuntimePaths) -> Result<(), Box<dyn std::error::Error>> {
    let path = paths.config_file();
    if path.exists() {
        load_bootstrap(paths)?;
        return Ok(());
    }
    let body = toml::to_string_pretty(&BootstrapConfig::default())?;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

pub fn load_bootstrap(paths: &RuntimePaths) -> Result<BootstrapConfig, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(paths.config_file())?;
    let config: BootstrapConfig = toml::from_str(&raw)?;
    validate_bootstrap(&config)?;
    Ok(config)
}

fn validate_bootstrap(config: &BootstrapConfig) -> Result<(), Box<dyn std::error::Error>> {
    config
        .server
        .http_address
        .parse::<std::net::SocketAddr>()
        .map_err(|error| format!("server.http_address is invalid: {error}"))?;
    config
        .server
        .grpc_address
        .parse::<std::net::SocketAddr>()
        .map_err(|error| format!("server.grpc_address is invalid: {error}"))?;
    tracing_subscriber::EnvFilter::try_new(&config.log.level)
        .map_err(|error| format!("log.level is invalid: {error}"))?;
    Ok(())
}

fn default_mcp_allowed_hosts() -> Vec<String> {
    vec![
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        "[::1]".to_owned(),
        "localhost:3000".to_owned(),
        "127.0.0.1:3000".to_owned(),
        "[::1]:3000".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_rejects_unknown_fields() {
        let raw = "[server]\nhttp_address='127.0.0.1:3000'\ngrpc_address='127.0.0.1:50051'\nextra=true\n[log]\nlevel='info'\noutput='file'\n";
        assert!(toml::from_str::<BootstrapConfig>(raw).is_err());
    }
}
