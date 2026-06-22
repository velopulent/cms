use std::path::PathBuf;

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Deserializer};

use crate::cli::Cli;
use crate::paths;
use crate::secrets;

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub database_url: String,
    pub bind_address: String,
    pub grpc_bind_address: String,

    // Filesystem storage
    pub storage_fs_path: Option<String>,

    // S3-compatible storage
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_public_url: Option<String>,

    // Backup destination + options (independent of the site-files storage above)
    pub backup_enabled: bool,
    pub backup_destination: String, // "filesystem" | "s3"
    pub backup_local_path: Option<String>,
    pub backup_zstd_level: i32,
    pub backup_default_retention: i64,
    pub backup_s3_access_key_id: Option<String>,
    pub backup_s3_secret_access_key: Option<String>,
    pub backup_s3_bucket: Option<String>,
    pub backup_s3_region: Option<String>,
    pub backup_s3_endpoint: Option<String>,
    pub backup_s3_public_url: Option<String>,
    /// AES-256 backup key (hex). From `BACKUP_ENCRYPTION_KEY` or persisted secrets.
    pub backup_encryption_key: Option<String>,

    // Upload limits
    pub max_upload_size_bytes: usize,

    // Cookie security
    pub cookie_secure: bool,
    pub session_lifetime_hours: i64,
    pub public_registration_enabled: bool,
    pub allowed_origins: Vec<String>,
    pub production: bool,

    // Database pool
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
    pub db_idle_timeout_secs: u64,

    // Rate limiting
    pub rate_limit_max_requests: u32,
    pub rate_limit_window_secs: u64,

    // Password hashing cost (bcrypt work factor)
    pub bcrypt_cost: u32,
    // Trust reverse-proxy client-IP headers (X-Forwarded-For / X-Real-IP)
    pub trust_proxy_headers: bool,
    // Allow webhooks to target private/loopback addresses (SSRF guard off)
    pub webhook_allow_private_targets: bool,

    // Hash secret for access token fast lookup
    pub hmac_secret: String,

    // Full-text search (Tantivy)
    pub search_enabled: bool,
    pub search_index_path: Option<String>,

    // MCP configuration
    pub mcp_enabled: bool,
    pub mcp_allowed_hosts: Vec<String>,
    pub mcp_allowed_origins: Vec<String>,
    pub public_url: Option<String>,

    // Logging
    pub log_level: String,
    pub log_output: String,
    pub log_format: String,
    pub log_annotations: bool,
    pub log_dir: String,
}

// `cms` is the library crate (handlers/services/etc.); `vcms` is the binary crate
// (main.rs lifecycle/startup logs). Both must be enabled to see all of our logs.
static DEFAULT_LOG_LEVEL: &str = "cms=debug,vcms=debug,tower_http=debug,axum=debug";

/// Intermediate, fully-optional config deserialized from the merged figment
/// layers (defaults < TOML file < env vars < CLI flags). Converted into the
/// runtime [`Config`] by [`RawConfig::into_config`], which applies hardcoded
/// defaults and the secret-warning behavior.
#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    database_url: Option<String>,
    hmac_secret: Option<String>,
    bind_address: Option<String>,
    grpc_bind_address: Option<String>,

    storage_fs_path: Option<String>,

    s3_access_key_id: Option<String>,
    s3_secret_access_key: Option<String>,
    s3_bucket: Option<String>,
    s3_region: Option<String>,
    s3_endpoint: Option<String>,
    s3_public_url: Option<String>,

    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    backup_enabled: Option<bool>,
    backup_destination: Option<String>,
    backup_local_path: Option<String>,
    backup_zstd_level: Option<i32>,
    backup_default_retention: Option<i64>,
    backup_s3_access_key_id: Option<String>,
    backup_s3_secret_access_key: Option<String>,
    backup_s3_bucket: Option<String>,
    backup_s3_region: Option<String>,
    backup_s3_endpoint: Option<String>,
    backup_s3_public_url: Option<String>,
    backup_encryption_key: Option<String>,

    max_upload_size_mb: Option<usize>,

    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    cookie_secure: Option<bool>,
    session_lifetime_hours: Option<i64>,
    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    public_registration_enabled: Option<bool>,
    #[serde(default, deserialize_with = "de_opt_str_list")]
    allowed_origins: Option<Vec<String>>,
    /// Maps the `VCMS_ENV` env var / `env` TOML key; "production" => production.
    env: Option<String>,

    db_max_connections: Option<u32>,
    db_min_connections: Option<u32>,
    db_acquire_timeout_secs: Option<u64>,
    db_idle_timeout_secs: Option<u64>,

    rate_limit_max_requests: Option<u32>,
    rate_limit_window_secs: Option<u64>,

    bcrypt_cost: Option<u32>,
    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    trust_proxy_headers: Option<bool>,
    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    webhook_allow_private_targets: Option<bool>,

    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    search_enabled: Option<bool>,
    search_index_path: Option<String>,

    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    mcp_enabled: Option<bool>,
    #[serde(default, deserialize_with = "de_opt_str_list")]
    mcp_allowed_hosts: Option<Vec<String>>,
    #[serde(default, deserialize_with = "de_opt_str_list")]
    mcp_allowed_origins: Option<Vec<String>>,
    public_url: Option<String>,

    log: Option<RawLog>,
}

#[derive(Debug, Default, Deserialize)]
struct RawLog {
    level: Option<String>,
    output: Option<String>,
    format: Option<String>,
    #[serde(default, deserialize_with = "de_opt_lenient_bool")]
    annotations: Option<bool>,
    dir: Option<String>,
}

impl Config {
    /// Load configuration by merging, in increasing precedence:
    /// built-in defaults < config file < environment variables < CLI flags.
    pub fn load(cli: &Cli) -> Result<Self, Box<figment::Error>> {
        let mut figment = Figment::new();

        // Lowest-precedence secret layer: persisted HMAC secret from
        // `~/.vcms/secrets.toml`. Read-only here (generation happens in
        // `secrets::ensure()` during serve/admin). Best-effort: a missing or
        // unreadable file just leaves the built-in defaults in play. Env vars
        // and CLI flags still override these.
        if let Ok(Some(persisted)) = secrets::load() {
            figment = figment.merge(Serialized::defaults(persisted));
        }

        if let Some(path) = resolve_config_path(cli) {
            figment = figment.merge(Toml::file(path));
        }

        // Environment layer: keep the existing unprefixed names. Remap the few
        // that don't match a field 1:1 (legacy log vars, VCMS_ENV) and project
        // log keys into the nested `log` table via the "." separator.
        figment = figment.merge(
            Env::raw()
                .map(|key| {
                    let k = key.as_str().to_ascii_lowercase();
                    match k.as_str() {
                        "rust_log" => "log.level".into(),
                        "log_output" => "log.output".into(),
                        "log_format" => "log.format".into(),
                        "log_annotations" => "log.annotations".into(),
                        "log_dir" => "log.dir".into(),
                        "vcms_env" => "env".into(),
                        _ => k.into(),
                    }
                })
                .split("."),
        );

        // CLI flag layer: highest precedence, only for flags the user passed.
        if let Some(v) = &cli.bind {
            figment = figment.merge(Serialized::default("bind_address", v));
        }
        if let Some(v) = &cli.database_url {
            figment = figment.merge(Serialized::default("database_url", v));
        }
        if let Some(v) = &cli.log_level {
            figment = figment.merge(Serialized::default("log.level", v));
        }

        figment.extract::<RawConfig>().map_err(Box::new)?.into_config()
    }

    /// Convenience for callers that only want env + defaults (no CLI flags).
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self::load(&Cli::default()).expect("Failed to load configuration")
    }

    pub fn has_s3(&self) -> bool {
        self.s3_access_key_id.is_some() && self.s3_secret_access_key.is_some() && self.s3_bucket.is_some()
    }

    /// Whether a dedicated S3 backup destination is fully configured.
    pub fn has_backup_s3(&self) -> bool {
        self.backup_s3_access_key_id.is_some()
            && self.backup_s3_secret_access_key.is_some()
            && self.backup_s3_bucket.is_some()
    }

    pub fn validate_security(&self) -> Result<(), String> {
        // The secret is always real here: config load hard-errors when no
        // secret is resolved, so there is no built-in placeholder to guard
        // against. We only enforce strength/cookie policy in production.
        if self.production {
            if self.hmac_secret.len() < 32 {
                return Err("HMAC_SECRET must be at least 32 bytes long".into());
            }
            if !self.cookie_secure {
                return Err("COOKIE_SECURE must be enabled in production".into());
            }
        }

        Ok(())
    }

    /// Render the effective configuration as TOML with secrets redacted.
    /// Used by `vcms config show`.
    pub fn redacted_toml(&self) -> String {
        format!(
            "# Effective configuration (secrets redacted)\n\
             database_url = \"{}\"\n\
             hmac_secret = \"{}\"\n\
             bind_address = \"{}\"\n\
             grpc_bind_address = \"{}\"\n\
             public_url = {}\n\
             env = \"{}\"\n\n\
             # Storage\n\
             storage_fs_path = {}\n\
             s3_access_key_id = {}\n\
             s3_secret_access_key = {}\n\
             s3_bucket = {}\n\
             s3_region = {}\n\
             s3_endpoint = {}\n\
             s3_public_url = {}\n\n\
             # Uploads\n\
             max_upload_size_mb = {}\n\n\
             # Security / sessions\n\
             cookie_secure = {}\n\
             session_lifetime_hours = {}\n\
             public_registration_enabled = {}\n\
             allowed_origins = {}\n\n\
             # Database pool\n\
             db_max_connections = {}\n\
             db_min_connections = {}\n\
             db_acquire_timeout_secs = {}\n\
             db_idle_timeout_secs = {}\n\n\
             # Rate limiting\n\
             rate_limit_max_requests = {}\n\
             rate_limit_window_secs = {}\n\
             trust_proxy_headers = {}\n\n\
             # Password hashing\n\
             bcrypt_cost = {}\n\n\
             # MCP\n\
             mcp_enabled = {}\n\
             mcp_allowed_hosts = {}\n\
             mcp_allowed_origins = {}\n\n\
             [log]\n\
             level = \"{}\"\n\
             output = \"{}\"\n\
             format = \"{}\"\n\
             annotations = {}\n\
             dir = \"{}\"\n",
            self.database_url,
            redact(&self.hmac_secret),
            self.bind_address,
            self.grpc_bind_address,
            opt_str(&self.public_url),
            if self.production { "production" } else { "development" },
            opt_str(&self.storage_fs_path),
            opt_secret(&self.s3_access_key_id),
            opt_secret(&self.s3_secret_access_key),
            opt_str(&self.s3_bucket),
            opt_str(&self.s3_region),
            opt_str(&self.s3_endpoint),
            opt_str(&self.s3_public_url),
            self.max_upload_size_bytes / (1024 * 1024),
            self.cookie_secure,
            self.session_lifetime_hours,
            self.public_registration_enabled,
            toml_list(&self.allowed_origins),
            self.db_max_connections,
            self.db_min_connections,
            self.db_acquire_timeout_secs,
            self.db_idle_timeout_secs,
            self.rate_limit_max_requests,
            self.rate_limit_window_secs,
            self.trust_proxy_headers,
            self.bcrypt_cost,
            self.mcp_enabled,
            toml_list(&self.mcp_allowed_hosts),
            toml_list(&self.mcp_allowed_origins),
            self.log_level,
            self.log_output,
            self.log_format,
            self.log_annotations,
            self.log_dir,
        )
    }
}

impl RawConfig {
    fn into_config(self) -> Result<Config, Box<figment::Error>> {
        // No silent placeholder: a secret must come from secrets.toml, config, or
        // the HMAC_SECRET env var. `serve`/`admin` generate one via
        // `secrets::ensure()` and `mcp stdio` guards on its presence, so this only
        // fires on a genuinely uninitialized instance.
        let hmac_secret = self.hmac_secret.ok_or_else(|| {
            Box::new(figment::Error::from(
                "No HMAC secret resolved; run `vcms serve` once to generate ~/.vcms/secrets.toml, \
                 or set HMAC_SECRET"
                    .to_string(),
            ))
        })?;

        let log = self.log.unwrap_or_default();

        Ok(Config {
            database_url: self.database_url.unwrap_or_else(paths::default_database_url),
            hmac_secret,
            bind_address: self.bind_address.unwrap_or_else(|| "0.0.0.0:3000".into()),
            grpc_bind_address: self.grpc_bind_address.unwrap_or_else(|| "0.0.0.0:50051".into()),
            storage_fs_path: self.storage_fs_path,
            s3_access_key_id: self.s3_access_key_id,
            s3_secret_access_key: self.s3_secret_access_key,
            s3_bucket: self.s3_bucket,
            s3_region: self.s3_region,
            s3_endpoint: self.s3_endpoint,
            s3_public_url: self.s3_public_url,
            backup_enabled: self.backup_enabled.unwrap_or(true),
            backup_destination: self.backup_destination.unwrap_or_else(|| "filesystem".into()),
            backup_local_path: self.backup_local_path,
            backup_zstd_level: self.backup_zstd_level.unwrap_or(12),
            backup_default_retention: self.backup_default_retention.unwrap_or(7),
            backup_s3_access_key_id: self.backup_s3_access_key_id,
            backup_s3_secret_access_key: self.backup_s3_secret_access_key,
            backup_s3_bucket: self.backup_s3_bucket,
            backup_s3_region: self.backup_s3_region,
            backup_s3_endpoint: self.backup_s3_endpoint,
            backup_s3_public_url: self.backup_s3_public_url,
            backup_encryption_key: self.backup_encryption_key,
            max_upload_size_bytes: self.max_upload_size_mb.unwrap_or(50) * 1024 * 1024,
            cookie_secure: self.cookie_secure.unwrap_or(false),
            session_lifetime_hours: self.session_lifetime_hours.unwrap_or(24),
            public_registration_enabled: self.public_registration_enabled.unwrap_or(false),
            allowed_origins: self.allowed_origins.unwrap_or_default(),
            production: self.env.map(|v| v.eq_ignore_ascii_case("production")).unwrap_or(false),
            db_max_connections: self.db_max_connections.unwrap_or(20),
            db_min_connections: self.db_min_connections.unwrap_or(2),
            db_acquire_timeout_secs: self.db_acquire_timeout_secs.unwrap_or(30),
            db_idle_timeout_secs: self.db_idle_timeout_secs.unwrap_or(600),
            rate_limit_max_requests: self.rate_limit_max_requests.unwrap_or(100),
            rate_limit_window_secs: self.rate_limit_window_secs.unwrap_or(60),
            bcrypt_cost: self.bcrypt_cost.unwrap_or(bcrypt::DEFAULT_COST),
            trust_proxy_headers: self.trust_proxy_headers.unwrap_or(false),
            webhook_allow_private_targets: self.webhook_allow_private_targets.unwrap_or(false),
            search_enabled: self.search_enabled.unwrap_or(true),
            search_index_path: self.search_index_path,
            mcp_enabled: self.mcp_enabled.unwrap_or(true),
            mcp_allowed_hosts: self.mcp_allowed_hosts.unwrap_or_else(default_mcp_allowed_hosts),
            mcp_allowed_origins: self.mcp_allowed_origins.unwrap_or_default(),
            public_url: self.public_url.map(|v| v.trim_end_matches('/').to_string()),
            log_level: log.level.unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_string()),
            log_output: log.output.unwrap_or_else(|| "stdout".into()),
            log_format: log.format.unwrap_or_else(|| "pretty".into()),
            log_annotations: log.annotations.unwrap_or(false),
            log_dir: log
                .dir
                .unwrap_or_else(|| paths::logs_dir().to_string_lossy().into_owned()),
        })
    }
}

/// Resolve the config file path. First match wins; a missing file is fine.
/// Order: `--config` / `VCMS_CONFIG` > `./vcms.toml` > user config dir > `/etc/vcms`.
pub fn resolve_config_path(cli: &Cli) -> Option<PathBuf> {
    if let Some(path) = &cli.config {
        return Some(path.clone());
    }
    config_search_paths().into_iter().find(|candidate| candidate.exists())
}

/// The ordered list of locations searched when no `--config` is given.
pub fn config_search_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("vcms.toml")];
    if let Some(path) = user_config_path() {
        paths.push(path);
    }
    paths.push(PathBuf::from("/etc/vcms/config.toml"));
    paths
}

/// The user-config location for the config file: `~/.vcms/config.toml`
/// (or `$VCMS_HOME/config.toml`).
pub fn user_config_path() -> Option<PathBuf> {
    Some(paths::config_file())
}

/// The scaffold written by `vcms config init` — non-secret keys only, with
/// secrets shown as commented env-var hints.
pub fn default_config_toml() -> String {
    let hosts = default_mcp_allowed_hosts()
        .iter()
        .map(|h| format!("\"{h}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "# CMS configuration. Non-secret settings live here; secrets stay in the\n\
         # environment (or a .env file). Precedence: CLI flag > env var > this file.\n\
         #\n\
         # Secrets are NOT read from this file by convention — set them via env:\n\
         #   DATABASE_URL, HMAC_SECRET,\n\
         #   S3_ACCESS_KEY_ID, S3_SECRET_ACCESS_KEY\n\n\
         bind_address = \"0.0.0.0:3000\"\n\
         grpc_bind_address = \"0.0.0.0:50051\"\n\
         # public_url = \"https://cms.example.com\"\n\
         # env = \"production\"\n\n\
         # --- Storage (filesystem) ---\n\
         # storage_fs_path = \"./storage\"\n\n\
         # --- Storage (S3, non-secret parts) ---\n\
         # s3_bucket = \"my-bucket\"\n\
         # s3_region = \"us-east-1\"\n\
         # s3_endpoint = \"https://s3.example.com\"\n\
         # s3_public_url = \"https://cdn.example.com\"\n\n\
         # --- Uploads ---\n\
         max_upload_size_mb = 50\n\n\
         # --- Security / sessions ---\n\
         cookie_secure = false\n\
         session_lifetime_hours = 24\n\
         public_registration_enabled = false\n\
         allowed_origins = []\n\n\
         # --- Database pool ---\n\
         db_max_connections = 10\n\
         db_min_connections = 2\n\
         db_acquire_timeout_secs = 30\n\
         db_idle_timeout_secs = 600\n\n\
         # --- Rate limiting ---\n\
         rate_limit_max_requests = 100\n\
         rate_limit_window_secs = 60\n\
         # Trust X-Forwarded-For / X-Real-IP for client IP (enable only behind a trusted proxy)\n\
         trust_proxy_headers = false\n\n\
         # --- Password hashing ---\n\
         bcrypt_cost = 12\n\n\
         # --- Webhooks ---\n\
         # Allow webhooks to target private/loopback addresses (SSRF guard off)\n\
         webhook_allow_private_targets = false\n\n\
         # --- Backups ---\n\
         # Run the scheduled-backup poller and allow on-demand backups.\n\
         backup_enabled = true\n\
         # Destination for backup artifacts: \"filesystem\" (default, under ~/.vcms/backups)\n\
         # or \"s3\". S3 credentials are secrets — set them via env (see below).\n\
         backup_destination = \"filesystem\"\n\
         # backup_local_path = \"./backups\"\n\
         backup_zstd_level = 12\n\
         backup_default_retention = 7\n\
         # S3 backup destination (non-secret parts; keep it in a SEPARATE account/bucket\n\
         # from your site-files S3 for blast-radius isolation):\n\
         # backup_s3_bucket = \"my-cms-backups\"\n\
         # backup_s3_region = \"us-east-1\"\n\
         # backup_s3_endpoint = \"https://s3.example.com\"\n\
         # Secrets via env: BACKUP_S3_ACCESS_KEY_ID, BACKUP_S3_SECRET_ACCESS_KEY,\n\
         # and BACKUP_ENCRYPTION_KEY (else auto-generated into secrets.toml).\n\n\
         # --- Full-text search (Tantivy) ---\n\
         # Build a local inverted index so entry search is ranked + tokenized.\n\
         # When false, search falls back to a basic SQL LIKE match.\n\
         search_enabled = true\n\
         # search_index_path = \"./search\"   # defaults to ~/.vcms/search\n\n\
         # --- MCP ---\n\
         mcp_enabled = true\n\
         mcp_allowed_hosts = [{hosts}]\n\
         mcp_allowed_origins = []\n\n\
         # --- Logging ---\n\
         [log]\n\
         level = \"{DEFAULT_LOG_LEVEL}\"\n\
         output = \"stdout\"   # stdout | file\n\
         format = \"pretty\"   # pretty | json\n\
         annotations = false  # include file + line numbers\n\
         dir = \"logs\"        # used when output = \"file\"\n",
    )
}

fn default_mcp_allowed_hosts() -> Vec<String> {
    vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "[::1]".to_string(),
        "localhost:3000".to_string(),
        "127.0.0.1:3000".to_string(),
        "[::1]:3000".to_string(),
    ]
}

/// Accepts booleans as `true`/`false`, integers (`0`/`1`), or strings
/// (`"true"`/`"1"`) for env back-compat.
fn de_opt_lenient_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Lenient {
        Bool(bool),
        Int(i64),
        Str(String),
    }
    Ok(match Option::<Lenient>::deserialize(deserializer)? {
        None => None,
        Some(Lenient::Bool(b)) => Some(b),
        Some(Lenient::Int(i)) => Some(i != 0),
        Some(Lenient::Str(s)) => Some(matches!(s.to_ascii_lowercase().as_str(), "1" | "true")),
    })
}

/// Accepts a list either as a TOML array or as a comma-separated string
/// (env back-compat with the previous CSV parsing).
fn de_opt_str_list<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum List {
        Vec(Vec<String>),
        Str(String),
    }
    Ok(match Option::<List>::deserialize(deserializer)? {
        None => None,
        Some(List::Vec(v)) => Some(v),
        Some(List::Str(s)) => Some(
            s.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect(),
        ),
    })
}

fn redact(secret: &str) -> String {
    if secret.is_empty() {
        "".into()
    } else {
        "***redacted***".into()
    }
}

fn opt_str(value: &Option<String>) -> String {
    match value {
        Some(v) => format!("\"{v}\""),
        None => "\"<unset>\"".to_string(),
    }
}

fn opt_secret(value: &Option<String>) -> String {
    match value {
        Some(_) => "\"***redacted***\"".to_string(),
        None => "\"<unset>\"".to_string(),
    }
}

fn toml_list(values: &[String]) -> String {
    let inner = values.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(", ");
    format!("[{inner}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s3_config() -> Config {
        Config {
            s3_access_key_id: Some("key".to_string()),
            s3_secret_access_key: Some("secret".to_string()),
            s3_bucket: Some("bucket".to_string()),
            s3_region: Some("us-east-1".to_string()),
            ..Config::default()
        }
    }

    #[test]
    fn test_has_s3_returns_true_when_all_s3_fields_are_set() {
        assert!(s3_config().has_s3());
    }

    #[test]
    fn test_has_s3_returns_false_when_access_key_id_is_missing() {
        let config = Config {
            s3_access_key_id: None,
            ..s3_config()
        };
        assert!(!config.has_s3());
    }

    #[test]
    fn test_has_s3_returns_false_when_secret_is_missing() {
        let config = Config {
            s3_secret_access_key: None,
            ..s3_config()
        };
        assert!(!config.has_s3());
    }

    #[test]
    fn test_has_s3_returns_false_when_bucket_is_missing() {
        let config = Config {
            s3_bucket: None,
            ..s3_config()
        };
        assert!(!config.has_s3());
    }

    #[test]
    fn test_config_default_values() {
        let config = Config {
            storage_fs_path: Some("/tmp/storage".to_string()),
            max_upload_size_bytes: 50 * 1024 * 1024,
            cookie_secure: true,
            ..Config::default()
        };

        assert!(!config.has_s3());
        assert_eq!(config.max_upload_size_bytes, 50 * 1024 * 1024);
        assert!(config.cookie_secure);
    }

    #[test]
    fn test_raw_config_into_config_applies_defaults() {
        let config = RawConfig {
            hmac_secret: Some("test-secret".to_string()),
            ..RawConfig::default()
        }
        .into_config()
        .expect("a supplied secret should produce a config");
        assert!(config.database_url.starts_with("sqlite://"));
        assert!(config.database_url.ends_with("vcms.db"));
        assert_eq!(config.bind_address, "0.0.0.0:3000");
        assert_eq!(config.max_upload_size_bytes, 50 * 1024 * 1024);
        assert_eq!(config.log_level, DEFAULT_LOG_LEVEL);
        assert_eq!(config.log_output, "stdout");
        assert!(config.mcp_enabled);
        assert!(!config.production);
    }

    #[test]
    fn test_into_config_requires_hmac_secret() {
        // No secret from any layer => hard error, never a silent placeholder.
        assert!(RawConfig::default().into_config().is_err());
    }

    #[test]
    fn test_env_maps_to_production() {
        let config = RawConfig {
            hmac_secret: Some("test-secret".to_string()),
            env: Some("production".to_string()),
            ..RawConfig::default()
        }
        .into_config()
        .expect("a supplied secret should produce a config");
        assert!(config.production);
    }
}
