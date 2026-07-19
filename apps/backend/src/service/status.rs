use std::process::Command;

use super::SERVICE_NAME;

pub fn print() -> Result<(), Box<dyn std::error::Error>> {
    let (manager, mut command) = native_command();
    let output = command
        .output()
        .map_err(|error| format!("cannot query {manager}: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let state = normalized_state(output.status.success(), &stdout, &stderr);
    println!("service={SERVICE_NAME}");
    println!("manager={manager}");
    println!("state={state}");
    if !stdout.is_empty() {
        println!("details={}", stdout.replace(['\r', '\n'], " "));
    } else if !stderr.is_empty() {
        println!("details={}", stderr.replace(['\r', '\n'], " "));
    }
    if state == "unknown" {
        return Err(format!("{manager} could not determine vcms service state").into());
    }
    Ok(())
}

/// Detect registration, not runtime state. Data-directory existence is never used.
pub fn is_installed() -> Result<bool, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        if [
            "/etc/systemd/system/vcms.service",
            "/lib/systemd/system/vcms.service",
            "/usr/lib/systemd/system/vcms.service",
        ]
        .iter()
        .any(|path| std::path::Path::new(path).is_file())
        {
            return Ok(true);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let path = std::path::Path::new("/Library/LaunchDaemons/com.velopulent.vcms.plist");
        return match std::fs::metadata(path) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        };
    }

    let (manager, mut command) = native_command();
    let output = command
        .output()
        .map_err(|error| format!("cannot query {manager} service registration: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout} {stderr}").to_ascii_lowercase();
    if text.contains("could not be found")
        || text.contains("not-found")
        || text.contains("no such process")
        || text.contains("failed to get unit file state")
        || text.contains("1060")
    {
        return Ok(false);
    }
    if output.status.success() || text.contains("loaded") || text.contains("running") || text.contains("stopped") {
        return Ok(true);
    }
    Err(format!(
        "{manager} could not determine whether vcms is installed: {}",
        stderr.trim()
    )
    .into())
}

fn normalized_state(success: bool, stdout: &str, stderr: &str) -> &'static str {
    let text = format!("{stdout} {stderr}").to_ascii_lowercase();
    if text.contains("running") || text.trim() == "active" {
        "running"
    } else if text.contains("stopped") || text.contains("inactive") || text.contains("not running") {
        "stopped"
    } else if text.contains("could not be found") || text.contains("not-found") || text.contains("no such process") {
        "not-installed"
    } else if success {
        "installed"
    } else {
        "unknown"
    }
}

#[cfg(target_os = "linux")]
fn native_command() -> (&'static str, Command) {
    let mut command = Command::new("systemctl");
    command.args([
        "show",
        SERVICE_NAME,
        "--property=ActiveState,SubState,LoadState",
        "--no-pager",
    ]);
    ("systemd", command)
}

#[cfg(target_os = "macos")]
fn native_command() -> (&'static str, Command) {
    let mut command = Command::new("launchctl");
    command.args(["print", "system/com.velopulent.vcms"]);
    ("launchd", command)
}

#[cfg(windows)]
fn native_command() -> (&'static str, Command) {
    let mut command = Command::new("sc.exe");
    command.args(["query", SERVICE_NAME]);
    ("scm", command)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn native_command() -> (&'static str, Command) {
    let mut command = Command::new("false");
    command.arg(SERVICE_NAME);
    ("unsupported", command)
}

#[cfg(test)]
mod tests {
    use super::normalized_state;

    #[test]
    fn normalizes_common_states() {
        assert_eq!(
            normalized_state(true, "ActiveState=active\nSubState=running", ""),
            "running"
        );
        assert_eq!(normalized_state(false, "ActiveState=inactive", ""), "stopped");
        assert_eq!(
            normalized_state(false, "", "Unit vcms.service could not be found"),
            "not-installed"
        );
    }
}
