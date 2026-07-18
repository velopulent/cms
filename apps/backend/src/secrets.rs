//! Restricted instance trust root.

use std::io::Write;

use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paths::RuntimePaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedSecrets {
    /// Root key used only to derive purpose-specific runtime keys.
    pub master_key: String,
    /// Independent key for encrypted backup artifacts.
    pub backup_encryption_key: String,
    /// External database URLs may contain credentials and therefore live here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,
}

pub fn load(paths: &RuntimePaths) -> Result<Option<PersistedSecrets>, Box<dyn std::error::Error>> {
    let path = paths.secrets_file();
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)?;
    let secrets: PersistedSecrets = toml::from_str(&contents)?;
    validate(&secrets)?;
    Ok(Some(secrets))
}

/// Create secrets only for a genuinely fresh instance. Existing data without its
/// trust root must stop for explicit recovery instead of silently changing keys.
pub fn ensure(paths: &RuntimePaths) -> Result<PersistedSecrets, Box<dyn std::error::Error>> {
    if let Some(existing) = load(paths)? {
        return Ok(existing);
    }
    if paths.database_file().exists() {
        return Err(format!(
            "{} is missing but {} already exists; restore secrets.toml or run `vcms secrets reset --yes`",
            paths.secrets_file().display(),
            paths.database_file().display()
        )
        .into());
    }

    let secrets = fresh(None);
    persist_new(paths, &secrets)?;
    Ok(secrets)
}

pub fn fresh(database_url: Option<String>) -> PersistedSecrets {
    PersistedSecrets {
        master_key: random_hex(32),
        backup_encryption_key: random_hex(32),
        database_url,
    }
}

pub fn replace(paths: &RuntimePaths, secrets: &PersistedSecrets) -> Result<(), Box<dyn std::error::Error>> {
    validate(secrets)?;
    let target = paths.secrets_file();
    let temp = target.with_extension("toml.tmp");
    write_restricted(&temp, &toml::to_string_pretty(secrets)?, false)?;
    #[cfg(not(windows))]
    std::fs::rename(&temp, &target)?;
    #[cfg(windows)]
    {
        std::fs::copy(&temp, &target)?;
        restrict_permissions(&target)?;
        std::fs::remove_file(&temp)?;
    }
    Ok(())
}

fn persist_new(paths: &RuntimePaths, secrets: &PersistedSecrets) -> Result<(), Box<dyn std::error::Error>> {
    write_restricted(&paths.secrets_file(), &toml::to_string_pretty(secrets)?, true)
}

fn write_restricted(path: &std::path::Path, body: &str, create_new: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut options = std::fs::OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(!create_new)
        .create_new(create_new);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    restrict_permissions(path)?;
    Ok(())
}

fn validate(secrets: &PersistedSecrets) -> Result<(), Box<dyn std::error::Error>> {
    for (name, value) in [
        ("master_key", secrets.master_key.as_str()),
        ("backup_encryption_key", secrets.backup_encryption_key.as_str()),
    ] {
        if value.len() != 64 || hex::decode(value).is_err() {
            return Err(format!("{name} must be exactly 32 bytes encoded as 64 hexadecimal characters").into());
        }
    }
    Ok(())
}

/// Derive a stable, domain-separated 32-byte key without exposing the root key to
/// consumers. The hexadecimal result preserves existing service interfaces.
pub fn derive_key_hex(master_key: &str, purpose: &str) -> String {
    let root = hex::decode(master_key).expect("validated master key");
    let mut digest = Sha256::new();
    digest.update(b"vcms-key-v1\0");
    digest.update(purpose.as_bytes());
    digest.update(b"\0");
    digest.update(root);
    hex::encode(digest.finalize())
}

fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(windows)]
fn restrict_permissions(path: &std::path::Path) -> std::io::Result<()> {
    crate::paths::harden_windows_acl(path, false)
}

#[cfg(not(any(unix, windows)))]
fn restrict_permissions(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_secrets_are_valid_and_domain_separated() {
        let secrets = fresh(None);
        validate(&secrets).unwrap();
        assert_ne!(
            derive_key_hex(&secrets.master_key, "access-token-index"),
            derive_key_hex(&secrets.master_key, "webhook-encryption")
        );
    }

    #[test]
    fn missing_secrets_are_not_recreated_over_existing_database() {
        let dir = tempfile::tempdir().unwrap();
        let paths = RuntimePaths::portable(dir.path());
        paths.ensure().unwrap();
        std::fs::write(paths.database_file(), b"db").unwrap();
        assert!(ensure(&paths).unwrap_err().to_string().contains("secrets reset"));
    }
}
