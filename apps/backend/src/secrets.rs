//! Persisted instance secrets (`~/.cms/secrets.toml`).
//!
//! On first `serve`/`admin`, random `JWT_SECRET` and `HMAC_SECRET` values are
//! generated and written to `secrets.toml` (perms `0600` on unix). Every later
//! process — including a `cms mcp stdio` child launched from an unknown working
//! directory — reads the *same* values, so site-token verification matches the
//! server that signed the token. Environment variables still override the file.
//!
//! These secrets intentionally live in a dedicated, restricted file rather than
//! `config.toml`: the TOML config is for non-secret settings, while this file is
//! machine-managed and never scaffolded by `cms config init`.

use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::paths;

/// Auto-generated secrets persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSecrets {
    pub jwt_secret: String,
    pub hmac_secret: String,
}

/// Read the persisted secrets file if it exists.
///
/// Returns `None` when the file is absent (a fresh, uninitialized instance) and
/// an error only when the file exists but cannot be read or parsed.
pub fn load() -> Result<Option<PersistedSecrets>, Box<dyn std::error::Error>> {
    let path = paths::secrets_file();
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)?;
    let secrets: PersistedSecrets = toml::from_str(&contents)?;
    Ok(Some(secrets))
}

/// Load existing secrets, or generate and persist new ones if absent.
///
/// Called by instance-owning commands (`serve`, `admin`) before loading config.
/// Read-only commands (`mcp stdio`) must use [`load`] instead and never create
/// the file.
pub fn ensure() -> Result<PersistedSecrets, Box<dyn std::error::Error>> {
    if let Some(existing) = load()? {
        return Ok(existing);
    }

    let secrets = PersistedSecrets {
        jwt_secret: random_hex(32),
        hmac_secret: random_hex(32),
    };

    let path = paths::secrets_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = toml::to_string_pretty(&secrets)?;
    std::fs::write(&path, body)?;
    restrict_permissions(&path)?;

    Ok(secrets)
}

/// Generate `n` random bytes, hex-encoded (so a 32-byte secret is 64 chars).
fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Tighten the secrets file to owner-only read/write where the platform supports
/// it. Best-effort on Windows (filesystem ACLs already restrict to the user).
#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_home<T>(value: &std::path::Path, f: impl FnOnce() -> T) -> T {
        use std::sync::Mutex;
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var_os(paths::CMS_HOME_ENV);
        // SAFETY: guarded by LOCK so no other test reads/writes the env concurrently.
        unsafe { std::env::set_var(paths::CMS_HOME_ENV, value) };
        let result = f();
        match previous {
            Some(v) => unsafe { std::env::set_var(paths::CMS_HOME_ENV, v) },
            None => unsafe { std::env::remove_var(paths::CMS_HOME_ENV) },
        }
        result
    }

    #[test]
    fn load_returns_none_when_absent() {
        let dir = tempfile::tempdir().expect("temp dir");
        with_home(dir.path(), || {
            assert!(load().expect("load ok").is_none());
        });
    }

    #[test]
    fn ensure_generates_then_reuses_secrets() {
        let dir = tempfile::tempdir().expect("temp dir");
        with_home(dir.path(), || {
            let first = ensure().expect("generate");
            assert_eq!(first.jwt_secret.len(), 64);
            assert_eq!(first.hmac_secret.len(), 64);
            assert_ne!(first.jwt_secret, first.hmac_secret);
            assert!(paths::secrets_file().exists());

            // A second call must return the persisted values, not regenerate.
            let second = ensure().expect("reuse");
            assert_eq!(first.jwt_secret, second.jwt_secret);
            assert_eq!(first.hmac_secret, second.hmac_secret);
        });
    }
}
