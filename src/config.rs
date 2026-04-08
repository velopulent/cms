use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub bind_address: String,

    // Filesystem storage
    pub storage_fs_path: Option<String>,

    // S3-compatible storage
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_public_url: Option<String>,

    // Upload limits
    pub max_upload_size_bytes: usize,

    // Cookie security
    pub cookie_secure: bool,

    // Database pool
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    pub db_acquire_timeout_secs: u64,
    pub db_idle_timeout_secs: u64,

    // Rate limiting
    pub rate_limit_max_requests: u32,
    pub rate_limit_window_secs: u64,

    // Hash secret for API key fast lookup
    pub hmac_secret: String,
}

static DEFAULT_JWT_SECRET: &str = "cms-jwt-secret-change-in-production";

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string());

        if jwt_secret == DEFAULT_JWT_SECRET {
            eprintln!("WARNING: Using default JWT secret. Set JWT_SECRET environment variable in production!");
        }

        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:cms.db".into());

        Self {
            jwt_secret,
            database_url,
            bind_address: env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".into()),
            storage_fs_path: env::var("STORAGE_FS_PATH").ok(),
            s3_access_key_id: env::var("S3_ACCESS_KEY_ID").ok(),
            s3_secret_access_key: env::var("S3_SECRET_ACCESS_KEY").ok(),
            s3_bucket: env::var("S3_BUCKET").ok(),
            s3_region: env::var("S3_REGION").ok(),
            s3_endpoint: env::var("S3_ENDPOINT").ok(),
            s3_public_url: env::var("S3_PUBLIC_URL").ok(),
            max_upload_size_bytes: env::var("MAX_UPLOAD_SIZE_MB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(50)
                * 1024
                * 1024,
            cookie_secure: env::var("COOKIE_SECURE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            db_min_connections: env::var("DB_MIN_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            db_acquire_timeout_secs: env::var("DB_ACQUIRE_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            db_idle_timeout_secs: env::var("DB_IDLE_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(600),
            rate_limit_max_requests: env::var("RATE_LIMIT_MAX_REQUESTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            rate_limit_window_secs: env::var("RATE_LIMIT_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            hmac_secret: env::var("HMAC_SECRET").unwrap_or_else(|_| {
                eprintln!("WARNING: Using default HMAC secret. Set HMAC_SECRET environment variable in production!");
                "cms-hmac-secret-change-in-production".to_string()
            }),
        }
    }

    pub fn has_s3(&self) -> bool {
        self.s3_access_key_id.is_some() && self.s3_secret_access_key.is_some() && self.s3_bucket.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_s3_returns_true_when_all_s3_fields_are_set() {
        let config = Config {
            database_url: "sqlite:cms.db".to_string(),
            jwt_secret: "secret".to_string(),
            bind_address: "0.0.0.0:3000".to_string(),
            storage_fs_path: None,
            s3_access_key_id: Some("key".to_string()),
            s3_secret_access_key: Some("secret".to_string()),
            s3_bucket: Some("bucket".to_string()),
            s3_region: Some("us-east-1".to_string()),
            s3_endpoint: None,
            s3_public_url: None,
            max_upload_size_bytes: 50 * 1024 * 1024,
            cookie_secure: false,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            hmac_secret: "hmac".to_string(),
        };

        assert!(config.has_s3());
    }

    #[test]
    fn test_has_s3_returns_false_when_access_key_id_is_missing() {
        let config = Config {
            database_url: "sqlite:cms.db".to_string(),
            jwt_secret: "secret".to_string(),
            bind_address: "0.0.0.0:3000".to_string(),
            storage_fs_path: None,
            s3_access_key_id: None,
            s3_secret_access_key: Some("secret".to_string()),
            s3_bucket: Some("bucket".to_string()),
            s3_region: Some("us-east-1".to_string()),
            s3_endpoint: None,
            s3_public_url: None,
            max_upload_size_bytes: 50 * 1024 * 1024,
            cookie_secure: false,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            hmac_secret: "hmac".to_string(),
        };

        assert!(!config.has_s3());
    }

    #[test]
    fn test_has_s3_returns_false_when_secret_is_missing() {
        let config = Config {
            database_url: "sqlite:cms.db".to_string(),
            jwt_secret: "secret".to_string(),
            bind_address: "0.0.0.0:3000".to_string(),
            storage_fs_path: None,
            s3_access_key_id: Some("key".to_string()),
            s3_secret_access_key: None,
            s3_bucket: Some("bucket".to_string()),
            s3_region: Some("us-east-1".to_string()),
            s3_endpoint: None,
            s3_public_url: None,
            max_upload_size_bytes: 50 * 1024 * 1024,
            cookie_secure: false,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            hmac_secret: "hmac".to_string(),
        };

        assert!(!config.has_s3());
    }

    #[test]
    fn test_has_s3_returns_false_when_bucket_is_missing() {
        let config = Config {
            database_url: "sqlite:cms.db".to_string(),
            jwt_secret: "secret".to_string(),
            bind_address: "0.0.0.0:3000".to_string(),
            storage_fs_path: None,
            s3_access_key_id: Some("key".to_string()),
            s3_secret_access_key: Some("secret".to_string()),
            s3_bucket: None,
            s3_region: Some("us-east-1".to_string()),
            s3_endpoint: None,
            s3_public_url: None,
            max_upload_size_bytes: 50 * 1024 * 1024,
            cookie_secure: false,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            hmac_secret: "hmac".to_string(),
        };

        assert!(!config.has_s3());
    }

    #[test]
    fn test_config_default_values() {
        let config = Config {
            database_url: "sqlite:cms.db".to_string(),
            jwt_secret: "default-secret".to_string(),
            bind_address: "0.0.0.0:3000".to_string(),
            storage_fs_path: Some("/tmp/storage".to_string()),
            s3_access_key_id: None,
            s3_secret_access_key: None,
            s3_bucket: None,
            s3_region: None,
            s3_endpoint: None,
            s3_public_url: None,
            max_upload_size_bytes: 50 * 1024 * 1024,
            cookie_secure: true,
            db_max_connections: 10,
            db_min_connections: 2,
            db_acquire_timeout_secs: 30,
            db_idle_timeout_secs: 600,
            rate_limit_max_requests: 100,
            rate_limit_window_secs: 60,
            hmac_secret: "hmac".to_string(),
        };

        assert!(!config.has_s3());
        assert_eq!(config.max_upload_size_bytes, 50 * 1024 * 1024);
        assert!(config.cookie_secure);
    }
}
