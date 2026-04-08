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
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:cms.db".into()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "cms-jwt-secret-change-in-production".into()),
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
        }
    }

    pub fn has_s3(&self) -> bool {
        self.s3_access_key_id.is_some()
            && self.s3_secret_access_key.is_some()
            && self.s3_bucket.is_some()
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
        };

        assert!(!config.has_s3());
        assert_eq!(config.max_upload_size_bytes, 50 * 1024 * 1024);
        assert!(config.cookie_secure);
    }
}
