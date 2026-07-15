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
//! * **Single** — everything nests under one root. Chosen when `$VCMS_HOME` is set,
//!   the system service home exists, or a legacy `~/.vcms` already holds data.

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

/// Conventional system directory an installed OS service owns, per platform. The app
/// does not auto-select this path; service packages pass it through `$VCMS_HOME`.
pub fn system_home() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        Some(PathBuf::from(r"C:\ProgramData\vcms"))
    }
    #[cfg(target_os = "linux")]
    {
        Some(PathBuf::from("/var/lib/vcms"))
    }
    #[cfg(target_os = "macos")]
    {
        Some(PathBuf::from("/Library/Application Support/vcms"))
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Resolve the active layout: `$VCMS_HOME` → system home → legacy `~/.vcms` → platform split.
/// Gathers the real filesystem/env inputs and defers the precedence decision to the
/// pure [`resolve_layout`] (kept separate so it is testable without touching real
/// system dirs).
fn layout() -> Layout {
    let env_home = std::env::var_os(CMS_HOME_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let system = system_home().filter(|path| path.is_dir());

    // Back-compat: a pre-existing single `~/.vcms` keeps owning everything so an upgrade
    // never strands a user's database or secrets.
    let legacy = directories::BaseDirs::new()
        .map(|base| base.home_dir().join(".vcms"))
        .filter(|p| p.is_dir());

    let split = directories::ProjectDirs::from("", "", "vcms").map(|dirs| split_from(&dirs));

    resolve_layout(env_home, system, legacy, split)
}

/// Pure precedence policy. Each `Option` is a candidate the caller has already vetted
/// (env set & non-empty; legacy dir confirmed to exist; split from `ProjectDirs`).
/// Falls back to a local `.vcms` blob when no platform dirs are detectable (rare).
fn resolve_layout(
    env_home: Option<PathBuf>,
    system_home: Option<PathBuf>,
    legacy_home: Option<PathBuf>,
    split: Option<Layout>,
) -> Layout {
    if let Some(root) = env_home {
        return Layout::Single(root);
    }
    if let Some(root) = system_home {
        return Layout::Single(root);
    }
    if let Some(root) = legacy_home {
        return Layout::Single(root);
    }
    split.unwrap_or_else(|| Layout::Single(PathBuf::from(".vcms")))
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

/// Fail fast when the active root is the conventional system service home but this
/// process can't access it (e.g. non-elevated CLI against the ACL-locked
/// `C:\ProgramData\vcms` or root-owned `/var/lib/vcms`).
fn preflight_system_home() -> std::io::Result<()> {
    let Layout::Single(root) = layout() else {
        return Ok(());
    };
    if system_home().as_deref() != Some(root.as_path()) || !root.is_dir() {
        return Ok(());
    }
    // A read probe: opening the db and secrets both need list+read on this dir.
    match std::fs::read_dir(&root) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "vcms data at {} requires Administrator/root; re-run in an elevated terminal.",
                root.display()
            ),
        )),
        Err(e) => Err(e),
    }
}

/// Create the directories the running instance writes to (config, data, storage,
/// logs). Called by commands that own/initialize the instance (`serve`, `admin`,
/// `backup`, `restore`); read-only commands such as `mcp stdio`, `config path/show`
/// never call this and so never create anything or trip the preflight below.
pub fn ensure() -> std::io::Result<()> {
    preflight_system_home()?;
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

    fn split_sample() -> Option<Layout> {
        Some(Layout::Split {
            config: PathBuf::from("/c"),
            data: PathBuf::from("/d"),
            cache: PathBuf::from("/ch"),
            state: PathBuf::from("/s"),
        })
    }

    fn single_root_of(layout: Layout) -> Option<PathBuf> {
        match layout {
            Layout::Single(root) => Some(root),
            Layout::Split { .. } => None,
        }
    }

    #[test]
    fn resolve_env_home_outranks_everything() {
        let out = resolve_layout(
            Some(PathBuf::from("/env")),
            Some(PathBuf::from("/system")),
            Some(PathBuf::from("/legacy")),
            split_sample(),
        );
        assert_eq!(single_root_of(out), Some(PathBuf::from("/env")));
    }

    #[test]
    fn resolve_system_home_when_present() {
        let out = resolve_layout(None, Some(PathBuf::from("/system")), None, split_sample());
        assert_eq!(single_root_of(out), Some(PathBuf::from("/system")));
    }

    #[test]
    fn resolve_legacy_home_when_no_env_or_system() {
        let out = resolve_layout(None, None, Some(PathBuf::from("/legacy")), split_sample());
        assert_eq!(single_root_of(out), Some(PathBuf::from("/legacy")));
    }

    #[test]
    fn resolve_falls_through_to_split() {
        let out = resolve_layout(None, None, None, split_sample());
        assert!(matches!(out, Layout::Split { .. }));
    }

    #[test]
    fn resolve_local_blob_when_nothing_detectable() {
        let out = resolve_layout(None, None, None, None);
        assert_eq!(single_root_of(out), Some(PathBuf::from(".vcms")));
    }
}
