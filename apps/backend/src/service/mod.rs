//! `vcms service` — install and manage the server as a native OS background
//! service (systemd on Linux, launchd on macOS, the Service Control Manager on
//! Windows).
//!
//! The pure definition builders ([`systemd_unit`], [`launchd_plist`]) and the
//! shared helpers live here so they compile and unit-test on every platform; the
//! side-effecting bits (writing unit files, shelling out to the service manager,
//! talking to the SCM) live in the per-OS submodules behind `#[cfg]`.

use crate::cli::{Cli, ServiceAction};
use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Service name registered with systemd / the Windows SCM.
pub const SERVICE_NAME: &str = "vcms";
/// Human-readable service description.
pub const SERVICE_DISPLAY_NAME: &str = "vcms headless CMS";
/// launchd job label (reverse-DNS-ish, kept short and unambiguous).
pub const LAUNCHD_LABEL: &str = "local.vcms";

/// Contents written to a freshly-created `<VCMS_HOME>/.env`. Empty of real values
/// so the SQLite default + auto-generated `secrets.toml` work with zero edits; the
/// commented keys show Postgres/MySQL/S3 users where their secrets go.
pub(crate) const ENV_TEMPLATE: &str = "\
# Environment for the vcms service (loaded from $VCMS_HOME/.env, mode 0600).
# The default SQLite database and auto-generated secrets need nothing here.
# Uncomment and set these only for Postgres/MySQL or S3 storage:
#
# DATABASE_URL=postgres://user:password@localhost/vcms
# S3_ACCESS_KEY_ID=...
# S3_SECRET_ACCESS_KEY=...
# S3_BUCKET=...
";

/// Resolved inputs for writing a service definition.
#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// OS account the service runs as (unix only; Windows uses LocalSystem).
    pub user: String,
    /// Absolute path to the `vcms` binary, used as the program to launch.
    pub exe_path: PathBuf,
    /// The account's CMS home directory (becomes `VCMS_HOME`).
    pub vcms_home: PathBuf,
}

impl InstallOptions {
    /// `<vcms_home>/.env` — the optional, 0600 env file the service loads.
    pub fn env_file(&self) -> PathBuf {
        self.vcms_home.join(".env")
    }
}

/// Render the systemd unit file for the resolved install options.
///
/// `EnvironmentFile=-` makes the `.env` optional (a missing/empty file is a no-op).
pub fn systemd_unit(opts: &InstallOptions) -> String {
    let exe = opts.exe_path.to_string_lossy();
    let home = opts.vcms_home.to_string_lossy();
    // systemd paths are always `/`-separated; build the env-file path explicitly so
    // the rendered unit is host-independent (don't use `PathBuf::join`, which would
    // emit a `\` when this runs on a non-unix host, e.g. in tests).
    let env_file = format!("{home}/.env");
    format!(
        "[Unit]\n\
         Description={SERVICE_DISPLAY_NAME}\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         User={user}\n\
         Environment=VCMS_HOME={home}\n\
         EnvironmentFile=-{env_file}\n\
         ExecStart={exe} serve\n\
         Restart=always\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        user = opts.user,
    )
}

/// Render the launchd LaunchDaemon plist for the resolved install options.
pub fn launchd_plist(opts: &InstallOptions) -> String {
    let exe = xml_escape(&opts.exe_path.to_string_lossy());
    let home = xml_escape(&opts.vcms_home.to_string_lossy());
    let user = xml_escape(&opts.user);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \x20 <key>Label</key>\n\
         \x20 <string>{LAUNCHD_LABEL}</string>\n\
         \x20 <key>ProgramArguments</key>\n\
         \x20 <array>\n\
         \x20   <string>{exe}</string>\n\
         \x20   <string>serve</string>\n\
         \x20 </array>\n\
         \x20 <key>UserName</key>\n\
         \x20 <string>{user}</string>\n\
         \x20 <key>EnvironmentVariables</key>\n\
         \x20 <dict>\n\
         \x20   <key>VCMS_HOME</key>\n\
         \x20   <string>{home}</string>\n\
         \x20 </dict>\n\
         \x20 <key>RunAtLoad</key>\n\
         \x20 <true/>\n\
         \x20 <key>KeepAlive</key>\n\
         \x20 <true/>\n\
         </dict>\n\
         </plist>\n",
    )
}

/// Minimal XML escaping for plist string values.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Validate that a username is safe to interpolate into a shell command / unit
/// file: POSIX-portable account names only. Guards the `echo ~name` home lookup.
pub fn validate_username(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 32 {
        return Err(format!("invalid username '{name}': must be 1-32 characters"));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_lowercase() || first == '_') {
        return Err(format!(
            "invalid username '{name}': must start with a lowercase letter or '_'"
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err(format!("invalid username '{name}': only [a-z0-9_-] allowed"));
    }
    Ok(())
}

/// Dispatch a `vcms service <action>` invocation to the platform implementation.
pub async fn run_service(action: &ServiceAction, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    // Exactly one of these `let` bindings is active per target, so `result` is the
    // single tail expression (no per-cfg `return`, which clippy flags as needless).
    #[cfg(target_os = "linux")]
    let result = linux::dispatch(action, cli);
    #[cfg(target_os = "macos")]
    let result = macos::dispatch(action, cli);
    #[cfg(windows)]
    let result = windows::dispatch(action, cli);
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    let result = {
        let _ = (action, cli);
        Err::<(), Box<dyn std::error::Error>>("`vcms service` is only supported on Linux, macOS, and Windows".into())
    };

    result
}

// ----- unix-shared helpers (compiled only on Linux + macOS) -----

/// Reject a path that is a symlink, **without following it** (`lstat`), so a
/// pre-planted symlink can't redirect a root-owned write. A dangling symlink (target
/// missing) is still caught — unlike `Path::exists()`, which follows the link and
/// reports `false`. A genuinely absent path is fine (returns `Ok`).
#[cfg(unix)]
pub(crate) fn reject_symlink(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!("refusing symlink at {}", path.display()).into()),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Atomically create `<VCMS_HOME>/.env` (mode 0600) with the env template, refusing
/// to follow a symlink. `create_new` (`O_EXCL | O_CREAT`) fails rather than follows if
/// the path already exists or is a symlink; an already-present regular file is left
/// as-is (we only re-check it isn't a symlink).
#[cfg(unix)]
pub(crate) fn create_secure_env_file(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    reject_symlink(path)?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(mut file) => file.write_all(ENV_TEMPLATE.as_bytes())?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => reject_symlink(path)?,
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Error if the current process is not running as root (euid 0).
#[cfg(unix)]
pub(crate) fn require_root(verb: &str) -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: `geteuid` is always safe — it only reads the calling process's id.
    let euid = unsafe { libc::geteuid() };
    if euid != 0 {
        return Err(format!("`vcms service {verb}` requires root (try: sudo vcms service {verb})").into());
    }
    Ok(())
}

/// Resolve the account the service should run as: explicit `--user`, else the real
/// user behind sudo (`$SUDO_USER`), else the current login — never root.
#[cfg(unix)]
pub(crate) fn resolve_run_user(explicit: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(user) = explicit {
        validate_username(user)?;
        if user == "root" {
            return Err(
                "refusing to run the service as root; pass --user <name> with an existing non-root account".into(),
            );
        }
        return Ok(user.to_string());
    }
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        if !sudo_user.is_empty() && sudo_user != "root" {
            validate_username(&sudo_user)?;
            return Ok(sudo_user);
        }
    }
    let current = std::env::var("USER").unwrap_or_default();
    if current.is_empty() || current == "root" {
        return Err("could not determine a non-root account to run the service as; \
                    pass --user <name> with an existing non-root account"
            .into());
    }
    validate_username(&current)?;
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> InstallOptions {
        InstallOptions {
            user: "deploy".into(),
            exe_path: PathBuf::from("/usr/local/bin/vcms"),
            vcms_home: PathBuf::from("/var/lib/vcms"),
        }
    }

    #[test]
    fn systemd_unit_contains_key_directives() {
        let unit = systemd_unit(&sample());
        assert!(unit.contains("ExecStart=/usr/local/bin/vcms serve"));
        assert!(unit.contains("User=deploy"));
        assert!(unit.contains("Environment=VCMS_HOME=/var/lib/vcms"));
        assert!(unit.contains("EnvironmentFile=-/var/lib/vcms/.env"));
        assert!(unit.contains("Restart=always"));
        assert!(unit.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn launchd_plist_contains_program_and_user() {
        let plist = launchd_plist(&sample());
        assert!(plist.contains("<string>local.vcms</string>"));
        assert!(plist.contains("<string>/usr/local/bin/vcms</string>"));
        assert!(plist.contains("<string>serve</string>"));
        assert!(plist.contains("<string>deploy</string>"));
        assert!(plist.contains("<key>VCMS_HOME</key>"));
        assert!(plist.contains("<string>/var/lib/vcms</string>"));
    }

    #[test]
    fn username_validation_rejects_injection() {
        assert!(validate_username("deploy").is_ok());
        assert!(validate_username("_svc-1").is_ok());
        assert!(validate_username("root").is_ok());
        assert!(validate_username("a; rm -rf /").is_err());
        assert!(validate_username("Bob").is_err());
        assert!(validate_username("").is_err());
        assert!(validate_username("1user").is_err());
    }
}
