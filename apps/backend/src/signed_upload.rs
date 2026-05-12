use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

const UPLOAD_TOKEN_EXPIRY_SECS: i64 = 900; // 15 minutes

#[derive(Debug, Clone)]
pub struct SignedUploadToken {
    pub file_id: String,
    pub site_id: String,
    pub filename: String,
    pub content_type: String,
    pub storage_provider: String,
    pub expires_at: i64,
    pub signature: String,
}

#[derive(Error, Debug)]
pub enum SignedUploadError {
    #[error("Token expired")]
    Expired,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid token format")]
    InvalidFormat,

    #[error("Base64 decode error: {0}")]
    Base64Error(String),
}

impl SignedUploadToken {
    pub fn generate(site_id: &str, filename: &str, content_type: &str, hmac_secret: &str) -> (Self, String) {
        let file_id = uuid::Uuid::now_v7().to_string();
        let storage_provider = "filesystem".to_string();
        let expires_at = chrono::Utc::now().timestamp() + UPLOAD_TOKEN_EXPIRY_SECS;

        let payload = format!(
            "{}:{}:{}:{}:{}:{}",
            file_id, site_id, filename, content_type, storage_provider, expires_at
        );

        let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let token = Self {
            file_id,
            site_id: site_id.to_string(),
            filename: filename.to_string(),
            content_type: content_type.to_string(),
            storage_provider,
            expires_at,
            signature,
        };

        let encoded = Self::encode(&token);
        (token, encoded)
    }

    pub fn generate_with_storage_provider(
        site_id: &str,
        filename: &str,
        content_type: &str,
        storage_provider: &str,
        hmac_secret: &str,
    ) -> (Self, String) {
        let file_id = uuid::Uuid::now_v7().to_string();
        let expires_at = chrono::Utc::now().timestamp() + UPLOAD_TOKEN_EXPIRY_SECS;

        let payload = format!(
            "{}:{}:{}:{}:{}:{}",
            file_id, site_id, filename, content_type, storage_provider, expires_at
        );

        let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let token = Self {
            file_id,
            site_id: site_id.to_string(),
            filename: filename.to_string(),
            content_type: content_type.to_string(),
            storage_provider: storage_provider.to_string(),
            expires_at,
            signature,
        };

        let encoded = Self::encode(&token);
        (token, encoded)
    }

    pub fn verify(token_str: &str, hmac_secret: &str) -> Result<Self, SignedUploadError> {
        let decoded = URL_SAFE_NO_PAD
            .decode(token_str)
            .map_err(|e| SignedUploadError::Base64Error(e.to_string()))?;

        let json_str = String::from_utf8(decoded).map_err(|_| SignedUploadError::InvalidFormat)?;

        let token: SignedUploadTokenInternal =
            serde_json::from_str(&json_str).map_err(|_| SignedUploadError::InvalidFormat)?;

        if token.expires_at < chrono::Utc::now().timestamp() {
            return Err(SignedUploadError::Expired);
        }

        let payload = format!(
            "{}:{}:{}:{}:{}:{}",
            token.file_id, token.site_id, token.filename, token.content_type, token.storage_provider, token.expires_at
        );

        let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        if !constant_time_eq(token.signature.as_bytes(), expected.as_bytes()) {
            return Err(SignedUploadError::InvalidSignature);
        }

        Ok(Self {
            file_id: token.file_id,
            site_id: token.site_id,
            filename: token.filename,
            content_type: token.content_type,
            storage_provider: token.storage_provider,
            expires_at: token.expires_at,
            signature: token.signature,
        })
    }

    pub fn expires_at(&self) -> String {
        chrono::DateTime::from_timestamp(self.expires_at, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default()
    }

    fn encode(token: &Self) -> String {
        let internal = SignedUploadTokenInternal {
            file_id: token.file_id.clone(),
            site_id: token.site_id.clone(),
            filename: token.filename.clone(),
            content_type: token.content_type.clone(),
            storage_provider: token.storage_provider.clone(),
            expires_at: token.expires_at,
            signature: token.signature.clone(),
        };
        let json = serde_json::to_string(&internal).expect("SignedUploadToken should be serializable");
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SignedUploadTokenInternal {
    file_id: String,
    site_id: String,
    filename: String,
    content_type: String,
    storage_provider: String,
    expires_at: i64,
    signature: String,
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify() {
        let secret = "test-hmac-secret";
        let (token, encoded) = SignedUploadToken::generate("site-123", "photo.jpg", "image/jpeg", secret);

        assert!(!encoded.is_empty());
        assert_eq!(token.site_id, "site-123");
        assert_eq!(token.filename, "photo.jpg");
        assert_eq!(token.content_type, "image/jpeg");

        let verified = SignedUploadToken::verify(&encoded, secret).unwrap();
        assert_eq!(verified.file_id, token.file_id);
        assert_eq!(verified.site_id, token.site_id);
        assert_eq!(verified.filename, token.filename);
    }

    #[test]
    fn test_verify_wrong_secret() {
        let (_, encoded) = SignedUploadToken::generate("site-123", "photo.jpg", "image/jpeg", "secret1");
        let result = SignedUploadToken::verify(&encoded, "secret2");
        assert!(matches!(result, Err(SignedUploadError::InvalidSignature)));
    }

    #[test]
    fn test_verify_corrupted_token() {
        let result = SignedUploadToken::verify("not-valid-base64!!", "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_with_storage_provider() {
        let (token, encoded) =
            SignedUploadToken::generate_with_storage_provider("site-123", "doc.pdf", "application/pdf", "s3", "secret");
        assert_eq!(token.storage_provider, "s3");

        let verified = SignedUploadToken::verify(&encoded, "secret").unwrap();
        assert_eq!(verified.storage_provider, "s3");
    }
}
