use std::path::Path;

use tokio::fs;

use crate::config::Config;

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("TLS enabled but certificate file not found: {0}")]
    CertFileNotFound(String),

    #[error("TLS enabled but key file not found: {0}")]
    KeyFileNotFound(String),

    #[error("Failed to read TLS file: {0}")]
    FileReadError(#[from] std::io::Error),

    #[error("Failed to load TLS configuration: {0}")]
    ConfigError(String),
}

/// Load axum-server TLS config from cert/key PEM files.
pub async fn load_axum_tls_config(config: &Config) -> Result<axum_server::tls_rustls::RustlsConfig, TlsError> {
    let cert_path = config
        .tls_cert_path
        .as_ref()
        .ok_or_else(|| TlsError::CertFileNotFound("TLS_CERT_PATH not set".into()))?;
    let key_path = config
        .tls_key_path
        .as_ref()
        .ok_or_else(|| TlsError::KeyFileNotFound("TLS_KEY_PATH not set".into()))?;

    if !Path::new(cert_path).exists() {
        return Err(TlsError::CertFileNotFound(cert_path.clone()));
    }
    if !Path::new(key_path).exists() {
        return Err(TlsError::KeyFileNotFound(key_path.clone()));
    }

    let cert_pem = fs::read(cert_path).await?;
    let key_pem = fs::read(key_path).await?;

    axum_server::tls_rustls::RustlsConfig::from_pem(cert_pem, key_pem)
        .await
        .map_err(|e| TlsError::ConfigError(e.to_string()))
}

/// Load tonic TLS identity from cert/key PEM files.
pub async fn load_tonic_identity(config: &Config) -> Result<tonic::transport::Identity, TlsError> {
    let cert_path = config
        .tls_cert_path
        .as_ref()
        .ok_or_else(|| TlsError::CertFileNotFound("TLS_CERT_PATH not set".into()))?;
    let key_path = config
        .tls_key_path
        .as_ref()
        .ok_or_else(|| TlsError::KeyFileNotFound("TLS_KEY_PATH not set".into()))?;

    let cert_pem = fs::read(cert_path).await?;
    let key_pem = fs::read(key_path).await?;

    Ok(tonic::transport::Identity::from_pem(cert_pem, key_pem))
}
