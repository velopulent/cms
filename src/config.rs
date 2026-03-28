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
        }
    }

    pub fn has_s3(&self) -> bool {
        self.s3_access_key_id.is_some()
            && self.s3_secret_access_key.is_some()
            && self.s3_bucket.is_some()
    }
}
