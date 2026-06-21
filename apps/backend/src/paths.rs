//! Resolution of the CMS home directory and the runtime files within it.
//!
//! Everything the running instance owns lives under a single root so a fresh
//! install "just works" regardless of the current working directory:
//!
//! ```text
//! ~/.cms/
//!   config.toml     # non-secret configuration
//!   secrets.toml    # auto-generated HMAC + backup encryption secrets (0600)
//!   cms.db          # default SQLite database (+ -wal / -shm)
//!   logs/           # rolling logs when log output = "file"
//!   storage/        # default filesystem storage for uploads
//! ```
//!
//! The root is `$CMS_HOME` when set, otherwise `~/.cms` resolved cross-platform
//! via the `directories` crate. This mirrors the convention used by tools like
//! `CARGO_HOME` / `PGDATA`: one predictable, overridable home.

use std::path::PathBuf;

/// Environment variable that overrides the home directory location.
pub const CMS_HOME_ENV: &str = "CMS_HOME";

/// The CMS home directory root.
///
/// `$CMS_HOME` wins if set and non-empty. Otherwise `~/.cms`. As a last resort
/// (no detectable home directory) falls back to `.cms` in the current dir.
pub fn home() -> PathBuf {
    if let Some(value) = std::env::var_os(CMS_HOME_ENV)
        && !value.is_empty()
    {
        return PathBuf::from(value);
    }

    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".cms"))
        .unwrap_or_else(|| PathBuf::from(".cms"))
}

/// `~/.cms/config.toml` — the user-level config file.
pub fn config_file() -> PathBuf {
    home().join("config.toml")
}

/// `~/.cms/secrets.toml` — auto-generated secrets file.
pub fn secrets_file() -> PathBuf {
    home().join("secrets.toml")
}

/// `~/.cms/cms.db` — the default SQLite database file.
pub fn default_db_path() -> PathBuf {
    home().join("cms.db")
}

/// `~/.cms/logs` — directory for rolling log files.
pub fn logs_dir() -> PathBuf {
    home().join("logs")
}

/// `~/.cms/storage` — default filesystem storage directory for uploads.
pub fn storage_dir() -> PathBuf {
    home().join("storage")
}

/// `~/.cms/backups` — default local destination for backup artifacts.
pub fn backups_dir() -> PathBuf {
    home().join("backups")
}

/// `~/.cms/search` — default location for the Tantivy full-text search index.
pub fn search_dir() -> PathBuf {
    home().join("search")
}

/// Build the default `DATABASE_URL` (`sqlite://<home>/cms.db`).
///
/// SQLite URLs use forward slashes, so backslashes are normalized for Windows.
pub fn default_database_url() -> String {
    format!("sqlite://{}", default_db_path().to_string_lossy().replace('\\', "/"))
}

/// Create the home directory and its subdirectories (`logs/`, `storage/`).
///
/// Called by commands that own/initialize the instance (`serve`, `admin`).
/// Read-only commands such as `mcp stdio` must not create anything.
pub fn ensure() -> std::io::Result<()> {
    let root = home();
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(logs_dir())?;
    std::fs::create_dir_all(storage_dir())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_home;

    #[test]
    fn cms_home_env_overrides_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        with_home(dir.path(), || {
            assert_eq!(home(), dir.path());
            assert_eq!(config_file(), dir.path().join("config.toml"));
            assert_eq!(secrets_file(), dir.path().join("secrets.toml"));
            assert_eq!(default_db_path(), dir.path().join("cms.db"));
            assert_eq!(logs_dir(), dir.path().join("logs"));
            assert_eq!(storage_dir(), dir.path().join("storage"));
        });
    }

    #[test]
    fn ensure_creates_subdirectories() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("home");
        with_home(&root, || {
            ensure().expect("ensure should create dirs");
            assert!(root.is_dir());
            assert!(logs_dir().is_dir());
            assert!(storage_dir().is_dir());
        });
    }

    #[test]
    fn default_database_url_uses_forward_slashes() {
        let dir = tempfile::tempdir().expect("temp dir");
        with_home(dir.path(), || {
            let url = default_database_url();
            assert!(url.starts_with("sqlite://"));
            assert!(!url.contains('\\'));
        });
    }
}
