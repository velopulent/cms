//! Deterministic runtime layout.
//!
//! Portable instances always use `<cwd>/vcms_data`. Installed instances always
//! use the platform service root. Selection is performed once from native service
//! registration and the resulting paths are passed through startup explicitly.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMode {
    Portable,
    Installed,
}

impl std::fmt::Display for RuntimeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Portable => "portable",
            Self::Installed => "installed",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePaths {
    mode: RuntimeMode,
    root: PathBuf,
}

impl RuntimePaths {
    pub fn portable(cwd: impl AsRef<Path>) -> Self {
        Self {
            mode: RuntimeMode::Portable,
            root: cwd.as_ref().join("vcms_data"),
        }
    }

    pub fn installed() -> Result<Self, String> {
        Ok(Self {
            mode: RuntimeMode::Installed,
            root: system_root().ok_or_else(|| "installed mode is unsupported on this platform".to_owned())?,
        })
    }

    pub fn for_mode(mode: RuntimeMode) -> Result<Self, String> {
        match mode {
            RuntimeMode::Portable => std::env::current_dir()
                .map(Self::portable)
                .map_err(|error| format!("cannot resolve current directory: {error}")),
            RuntimeMode::Installed => Self::installed(),
        }
    }

    pub fn mode(&self) -> RuntimeMode {
        self.mode
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn secrets_file(&self) -> PathBuf {
        self.root.join("secrets.toml")
    }

    pub fn database_file(&self) -> PathBuf {
        self.root.join("vcms.db")
    }

    pub fn storage_dir(&self) -> PathBuf {
        self.root.join("storage")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn search_dir(&self) -> PathBuf {
        self.root.join("search")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn database_url(&self) -> String {
        format!("sqlite://{}", self.database_file().to_string_lossy().replace('\\', "/"))
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        create_private_dir(&self.root)?;
        for path in [
            self.storage_dir(),
            self.backups_dir(),
            self.search_dir(),
            self.logs_dir(),
        ] {
            create_private_dir(&path)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(windows)]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    harden_windows_acl(path, true)
}

#[cfg(windows)]
pub(crate) fn harden_windows_acl(path: &Path, directory: bool) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    let whoami = std::process::Command::new("whoami.exe").output()?;
    if !whoami.status.success() {
        return Err(Error::other("whoami failed while hardening ACL"));
    }
    let user = String::from_utf8(whoami.stdout).map_err(|error| Error::new(ErrorKind::InvalidData, error))?;
    let user = user.trim();
    let suffix = if directory { ":(OI)(CI)F" } else { ":F" };
    let grants = [
        format!("{user}{suffix}"),
        format!("*S-1-5-18{suffix}"),
        format!("*S-1-5-32-544{suffix}"),
    ];
    let status = std::process::Command::new("icacls.exe")
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .args(grants)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::other("icacls failed while hardening ACL"))
    }
}

#[cfg(not(any(unix, windows)))]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(unix)]
pub fn permissions_secure(path: &Path, expected: u32) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o777 == expected)
}

#[cfg(windows)]
pub fn permissions_secure(path: &Path, _expected: u32) -> bool {
    let Ok(output) = std::process::Command::new("icacls.exe").arg(path).output() else {
        return false;
    };
    let acl = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    output.status.success()
        && !acl.contains("(i)")
        && !acl.contains("everyone:")
        && !acl.contains("authenticated users:")
        && !acl.contains("builtin\\users:")
}

#[cfg(not(any(unix, windows)))]
pub fn permissions_secure(_path: &Path, _expected: u32) -> bool {
    true
}

pub fn system_root() -> Option<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_layout_is_one_directory_under_cwd() {
        let paths = RuntimePaths::portable(Path::new("/work/site"));
        assert_eq!(paths.root(), Path::new("/work/site/vcms_data"));
        assert_eq!(paths.config_file(), Path::new("/work/site/vcms_data/config.toml"));
        assert_eq!(paths.database_file(), Path::new("/work/site/vcms_data/vcms.db"));
        assert_eq!(paths.storage_dir(), Path::new("/work/site/vcms_data/storage"));
    }

    #[test]
    fn sqlite_url_uses_forward_slashes() {
        let paths = RuntimePaths::portable(Path::new(r"C:\work"));
        assert!(!paths.database_url().contains('\\'));
    }
}
