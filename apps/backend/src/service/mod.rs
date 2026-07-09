//! `vcms service run` — the hidden entry point the Windows Service Control Manager
//! launches to host the server inside a native Windows service.
//!
//! All install/registration logic (systemd unit, launchd plist, SCM registration,
//! directory + ACL hardening) now lives in the platform installers (.deb / .rpm /
//! .pkg / .msi), not the binary. Only the Windows SCM host remains here because a
//! native Windows service must hand its thread to the SCM dispatcher.

/// Service name registered with the Windows SCM.
pub const SERVICE_NAME: &str = "vcms";
/// Human-readable service description.
pub const SERVICE_DISPLAY_NAME: &str = "vcms headless CMS";

#[cfg(windows)]
mod windows;

/// Dispatch a `vcms service <action>` invocation to the platform implementation.
#[cfg(windows)]
pub async fn run_service(
    action: &crate::cli::ServiceAction,
    cli: &crate::cli::Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    windows::dispatch(action, cli)
}
