//! Resolution of where the CMS keeps its runtime files.
//!
//! There are two layouts:
//!
//! * **Split** (the default for an interactive install) — files land in the
//!   platform-conventional per-type directories via the `directories` crate:
//!   config/secrets in the config dir, the database/uploads/backups in the data dir,
//!   the (rebuildable) search index in the cache dir, and logs in the state dir.
//!   On Linux this is XDG (`~/.config/vcms`, `~/.local/share/vcms`, `~/.cache/vcms`,
//!   `~/.local/state/vcms`); on macOS `~/Library/Application Support/vcms` (+ Caches);
//!   on Windows `%APPDATA%`/`%LOCALAPPDATA%` under `vcms`.
//!
//! * **Single** — everything nests under one root. Chosen when `$VCMS_HOME` is set
//!   (the OS-service installer pins it to a system dir like `/var/lib/vcms` or
//!   `C:\ProgramData\vcms`), or when a legacy `~/.vcms` already holds data (so an
//!   existing install keeps working untouched).

use std::path::PathBuf;

/// Environment variable that forces the single-directory layout at a chosen root.
pub const CMS_HOME_ENV: &str = "VCMS_HOME";

/// Where each class of file lives. Resolved fresh on each call (cheap) so tests and
/// the service can change `$VCMS_HOME` without a process restart.
enum Layout {
    /// One root holding everything (`$VCMS_HOME`, a legacy `~/.vcms`, or a `.vcms`
    /// fallback when no platform dirs are detectable).
    Single(PathBuf),
    /// Per-type platform directories.
    Split {
        config: PathBuf,
        data: PathBuf,
        cache: PathBuf,
        state: PathBuf,
    },
}

impl Layout {
    fn config(&self) -> PathBuf {
        match self {
            Layout::Single(root) => root.clone(),
            Layout::Split { config, .. } => config.clone(),
        }
    }

    fn data(&self) -> PathBuf {
        match self {
            Layout::Single(root) => root.clone(),
            Layout::Split { data, .. } => data.clone(),
        }
    }

    fn cache(&self) -> PathBuf {
        match self {
            Layout::Single(root) => root.clone(),
            Layout::Split { cache, .. } => cache.clone(),
        }
    }

    fn state(&self) -> PathBuf {
        match self {
            Layout::Single(root) => root.clone(),
            Layout::Split { state, .. } => state.clone(),
        }
    }
}

/// Resolve the active layout: `$VCMS_HOME` → legacy `~/.vcms` → platform split.
fn layout() -> Layout {
    if let Some(value) = std::env::var_os(CMS_HOME_ENV)
        && !value.is_empty()
    {
        return Layout::Single(PathBuf::from(value));
    }

    if let Some(base) = directories::BaseDirs::new() {
        // Back-compat: a pre-existing single `~/.vcms` keeps owning everything so an
        // upgrade never strands a user's database or secrets.
        let legacy = base.home_dir().join(".vcms");
        if legacy.is_dir() {
            return Layout::Single(legacy);
        }
    }

    match directories::ProjectDirs::from("", "", "vcms") {
        Some(dirs) => split_from(&dirs),
        // No detectable platform dirs (rare): fall back to a local blob.
        None => Layout::Single(PathBuf::from(".vcms")),
    }
}

/// Map `ProjectDirs` onto our four directory classes. Logs use the state dir where
/// the platform has one (Linux), else the local data dir (macOS/Windows).
fn split_from(dirs: &directories::ProjectDirs) -> Layout {
    Layout::Split {
        config: dirs.config_dir().to_path_buf(),
        data: dirs.data_dir().to_path_buf(),
        cache: dirs.cache_dir().to_path_buf(),
        state: dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir()).to_path_buf(),
    }
}

/// `config.toml` — the non-secret config file (config dir).
pub fn config_file() -> PathBuf {
    layout().config().join("config.toml")
}

/// `secrets.toml` — auto-generated secrets file (config dir, 0600 on unix).
pub fn secrets_file() -> PathBuf {
    layout().config().join("secrets.toml")
}

/// `.env` — optional environment file loaded at startup (config dir).
pub fn env_file() -> PathBuf {
    layout().config().join(".env")
}

/// `vcms.db` — the default SQLite database file (data dir).
pub fn default_db_path() -> PathBuf {
    layout().data().join("vcms.db")
}

/// `storage/` — default filesystem storage directory for uploads (data dir).
pub fn storage_dir() -> PathBuf {
    layout().data().join("storage")
}

/// `backups/` — default local destination for backup artifacts (data dir).
pub fn backups_dir() -> PathBuf {
    layout().data().join("backups")
}

/// `search/` — default location for the derived Tantivy index (cache dir).
pub fn search_dir() -> PathBuf {
    layout().cache().join("search")
}

/// `logs/` — directory for rolling log files (state dir).
pub fn logs_dir() -> PathBuf {
    layout().state().join("logs")
}

/// Build the default `DATABASE_URL` (`sqlite://<data>/vcms.db`).
///
/// SQLite URLs use forward slashes, so backslashes are normalized for Windows.
pub fn default_database_url() -> String {
    format!("sqlite://{}", default_db_path().to_string_lossy().replace('\\', "/"))
}

/// Create the directories the running instance writes to (config, data, storage,
/// logs). Called by commands that own/initialize the instance (`serve`, `admin`);
/// read-only commands such as `mcp stdio` must not create anything.
pub fn ensure() -> std::io::Result<()> {
    let layout = layout();
    for dir in [
        layout.config(),
        layout.data(),
        layout.state().join("logs"),
        layout.data().join("storage"),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Whether a file sits inside the home root in single-dir mode. Only meaningful for
/// `$VCMS_HOME`/legacy installs; in split mode there is no single root.
pub fn single_root() -> Option<PathBuf> {
    match layout() {
        Layout::Single(root) => Some(root),
        Layout::Split { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_home;
    use std::path::Path;

    #[test]
    fn vcms_home_forces_single_layout() {
        let dir = tempfile::tempdir().expect("temp dir");
        with_home(dir.path(), || {
            assert_eq!(single_root().as_deref(), Some(dir.path()));
            assert_eq!(config_file(), dir.path().join("config.toml"));
            assert_eq!(secrets_file(), dir.path().join("secrets.toml"));
            assert_eq!(env_file(), dir.path().join(".env"));
            assert_eq!(default_db_path(), dir.path().join("vcms.db"));
            assert_eq!(storage_dir(), dir.path().join("storage"));
            assert_eq!(backups_dir(), dir.path().join("backups"));
            assert_eq!(search_dir(), dir.path().join("search"));
            assert_eq!(logs_dir(), dir.path().join("logs"));
        });
    }

    #[test]
    fn ensure_creates_subdirectories_in_single_mode() {
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

    #[test]
    fn split_layout_separates_file_classes() {
        // Drive the pure mapping directly so the assertion is platform-independent
        // (real ProjectDirs vary by OS and ambient env).
        let layout = Layout::Split {
            config: PathBuf::from("/cfg"),
            data: PathBuf::from("/dat"),
            cache: PathBuf::from("/cch"),
            state: PathBuf::from("/st"),
        };
        assert!(layout.config().join("config.toml").starts_with(Path::new("/cfg")));
        assert!(layout.data().join("vcms.db").starts_with(Path::new("/dat")));
        assert!(layout.cache().join("search").starts_with(Path::new("/cch")));
        assert!(layout.state().join("logs").starts_with(Path::new("/st")));
        // config / data / cache / state are genuinely distinct in split mode.
        assert_ne!(layout.config(), layout.data());
        assert_ne!(layout.data(), layout.cache());
        assert_ne!(layout.cache(), layout.state());
    }

    #[test]
    fn split_state_falls_back_to_local_data_when_absent() {
        // When a platform lacks a state dir, logs must still resolve (we feed the
        // local-data dir into `state`). Modeled here as state == data.
        let layout = Layout::Split {
            config: PathBuf::from("/cfg"),
            data: PathBuf::from("/dat"),
            cache: PathBuf::from("/cch"),
            state: PathBuf::from("/dat"),
        };
        assert!(layout.state().join("logs").starts_with(Path::new("/dat")));
    }
}
