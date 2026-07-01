//! systemd integration for `vcms service` on Linux.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{InstallOptions, SERVICE_NAME, require_root, resolve_run_user, systemd_unit};
use crate::cli::{Cli, ServiceAction};

/// Where the generated unit file is written.
const UNIT_PATH: &str = "/etc/systemd/system/vcms.service";

/// System data directory the daemon owns (FHS convention for service state). Defined
/// once in `paths::system_home()` so a plain CLI invocation resolves to the same store;
/// always `Some` on Linux.
fn service_home() -> PathBuf {
    crate::paths::system_home().expect("system_home is always set on Linux")
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
    if !Path::new("/run/systemd/system").exists() {
        return Err("systemd was not detected (no /run/systemd/system). Only systemd-based \
                    Linux distributions are supported by `vcms service`."
            .into());
    }

    let user = resolve_run_user(user)?;
    // The daemon stores everything under one system dir (VCMS_HOME single-layout),
    // owned by the run-as account — not that account's per-user `~/.config` etc.
    let vcms_home = service_home();
    let exe_path = std::env::current_exe()?;
    let opts = InstallOptions {
        user: user.clone(),
        exe_path,
        vcms_home,
    };

    prepare_home(&opts)?;

    std::fs::write(UNIT_PATH, systemd_unit(&opts))?;
    println!("Wrote {UNIT_PATH}");

    run(Command::new("systemctl").arg("daemon-reload"))?;
    run(Command::new("systemctl").args(["enable", "--now", SERVICE_NAME]))?;

    println!("Service '{SERVICE_NAME}' installed, enabled at boot, and started (running as {user}).");
    println!("Secrets (Postgres/S3 only) go in {}", opts.env_file().display());
    println!("Check it with:  vcms service status");
    Ok(())
}

fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    require_root("uninstall")?;
    // Best-effort: the service may already be stopped/disabled.
    let _ = run(Command::new("systemctl").args(["disable", "--now", SERVICE_NAME]));
    if Path::new(UNIT_PATH).exists() {
        std::fs::remove_file(UNIT_PATH)?;
        println!("Removed {UNIT_PATH}");
    }
    run(Command::new("systemctl").arg("daemon-reload"))?;
    println!(
        "Service '{SERVICE_NAME}' uninstalled. Your data under {} was left intact.",
        service_home().display()
    );
    Ok(())
}

fn start() -> Result<(), Box<dyn std::error::Error>> {
    require_root("start")?;
    run(Command::new("systemctl").args(["start", SERVICE_NAME]))?;
    println!("Service '{SERVICE_NAME}' started.");
    Ok(())
}

fn stop() -> Result<(), Box<dyn std::error::Error>> {
    require_root("stop")?;
    run(Command::new("systemctl").args(["stop", SERVICE_NAME]))?;
    println!("Service '{SERVICE_NAME}' stopped.");
    Ok(())
}

fn status() -> Result<(), Box<dyn std::error::Error>> {
    let enabled = output_trim(Command::new("systemctl").args(["is-enabled", SERVICE_NAME]));
    let active = output_trim(Command::new("systemctl").args(["is-active", SERVICE_NAME]));
    println!("Service: {SERVICE_NAME}");
    println!("  enabled at boot: {}", enabled.as_deref().unwrap_or("unknown"));
    println!("  running:         {}", active.as_deref().unwrap_or("unknown"));
    // Detailed view (best-effort; ignored if the unit is absent).
    let _ = Command::new("systemctl")
        .args(["status", SERVICE_NAME, "--no-pager"])
        .status();
    Ok(())
}

/// Create the service home (`/var/lib/vcms`) + optional `.env` (0600) and hand
/// ownership to the run-as account so the daemon can read and write it.
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

fn run(cmd: &mut Command) -> Result<(), Box<dyn std::error::Error>> {
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("command {cmd:?} failed with {status}").into());
    }
    Ok(())
}

fn output_trim(cmd: &mut Command) -> Option<String> {
    let out = cmd.output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}
