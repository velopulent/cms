use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCursor {
    pub version: u8,
    pub page: i64,
    pub fingerprint: String,
}

pub fn fingerprint<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

pub fn encode(cursor: &PageCursor, secret: &str) -> String {
    let payload = serde_json::to_vec(cursor).expect("page cursor is serializable");
    let encoded = URL_SAFE_NO_PAD.encode(payload);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts every key length");
    mac.update(encoded.as_bytes());
    format!("{}.{}", encoded, hex::encode(mac.finalize().into_bytes()))
}

pub fn decode(value: &str, secret: &str) -> Result<PageCursor, &'static str> {
    let (payload, signature) = value.split_once('.').ok_or("invalid cursor")?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| "invalid cursor")?;
    mac.update(payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if signature.len() != expected.len() || !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return Err("invalid cursor");
    }
    let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|_| "invalid cursor")?;
    let cursor = serde_json::from_slice::<PageCursor>(&bytes).map_err(|_| "invalid cursor")?;
    if cursor.version != 1 || cursor.page < 1 {
        return Err("invalid cursor");
    }
    Ok(cursor)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().zip(right).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_tampering() {
        let cursor = PageCursor {
            version: 1,
            page: 4,
            fingerprint: "abc".into(),
        };
        let encoded = encode(&cursor, "secret");
        assert_eq!(decode(&encoded, "secret").unwrap().page, 4);
        assert!(decode(&format!("{}x", encoded), "secret").is_err());
    }
}
