//! launchd integration for `vcms service` on macOS (system LaunchDaemon).

use std::path::Path;
use std::process::Command;

use super::{InstallOptions, LAUNCHD_LABEL, launchd_plist, require_root, resolve_run_user, user_home};
use crate::cli::{Cli, ServiceAction};

/// Where the generated LaunchDaemon plist is written.
const PLIST_PATH: &str = "/Library/LaunchDaemons/local.vcms.plist";

/// Modern launchctl service target (`system/<label>`).
fn target() -> String {
    format!("system/{LAUNCHD_LABEL}")
}

pub fn dispatch(action: &ServiceAction, _cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ServiceAction::Install { user } => install(user.as_deref()),
        ServiceAction::Uninstall => uninstall(),
        ServiceAction::Status => status(),
        ServiceAction::Start => start(),
        ServiceAction::Stop => stop(),
    }
}

fn install(user: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    require_root("install")?;
    let user = resolve_run_user(user)?;
    let vcms_home = user_home(&user)?.join(".vcms");
    let exe_path = std::env::current_exe()?;
    let opts = InstallOptions {
        user: user.clone(),
        exe_path,
        vcms_home,
    };

    prepare_home(&opts)?;

    std::fs::write(PLIST_PATH, launchd_plist(&opts))?;
    // launchd refuses daemons whose plist is not root-owned / group- or world-writable.
    set_mode_644(Path::new(PLIST_PATH))?;
    run(Command::new("chown").arg("root:wheel").arg(PLIST_PATH))?;
    println!("Wrote {PLIST_PATH}");

    // bootstrap loads the job; enable persists it across reboots; kickstart starts now.
    run(Command::new("launchctl").args(["bootstrap", "system", PLIST_PATH]))?;
    let _ = run(Command::new("launchctl").args(["enable", &target()]));
    run(Command::new("launchctl").args(["kickstart", "-k", &target()]))?;

    println!("Service '{LAUNCHD_LABEL}' installed, enabled at boot, and started (running as {user}).");
    println!("Secrets (Postgres/S3 only) go in {}", opts.env_file().display());
    println!("Check it with:  vcms service status");
    Ok(())
}

fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    require_root("uninstall")?;
    // Best-effort: the job may already be unloaded.
    let _ = run(Command::new("launchctl").args(["bootout", &target()]));
    if Path::new(PLIST_PATH).exists() {
        std::fs::remove_file(PLIST_PATH)?;
        println!("Removed {PLIST_PATH}");
    }
    println!("Service '{LAUNCHD_LABEL}' uninstalled. Your data under ~/.vcms was left intact.");
    Ok(())
}

fn start() -> Result<(), Box<dyn std::error::Error>> {
    require_root("start")?;
    run(Command::new("launchctl").args(["kickstart", &target()]))?;
    println!("Service '{LAUNCHD_LABEL}' started.");
    Ok(())
}

fn stop() -> Result<(), Box<dyn std::error::Error>> {
    require_root("stop")?;
    run(Command::new("launchctl").args(["kill", "SIGTERM", &target()]))?;
    println!("Service '{LAUNCHD_LABEL}' stop signal sent.");
    Ok(())
}

fn status() -> Result<(), Box<dyn std::error::Error>> {
    println!("Service: {LAUNCHD_LABEL}");
    // `launchctl print` prints state, last exit status, and PID when running.
    let printed = Command::new("launchctl").args(["print", &target()]).status();
    if !matches!(printed, Ok(s) if s.success()) {
        println!("  (not loaded — run `vcms service install`)");
    }
    Ok(())
}

/// Create the run-as user's `~/.vcms` + optional `.env` (0600), owned by that user.
fn prepare_home(opts: &InstallOptions) -> Result<(), Box<dyn std::error::Error>> {
    super::reject_symlink(&opts.vcms_home)?;
    std::fs::create_dir_all(&opts.vcms_home)?;
    let env_file = opts.env_file();
    super::create_secure_env_file(&env_file)?;
    set_mode_600(&env_file)?;
    run(Command::new("chown")
        .arg("-R")
        .arg(format!("{}:", opts.user))
        .arg(&opts.vcms_home))?;
    Ok(())
}

fn set_mode_600(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

fn set_mode_644(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
}

fn run(cmd: &mut Command) -> Result<(), Box<dyn std::error::Error>> {
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("command {cmd:?} failed with {status}").into());
    }
    Ok(())
}
