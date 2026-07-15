use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::model::{Context, Result, TargetOs};
use crate::shared::{config_sample, copy_binary, reset_dir, run_cmd, write_file};

pub fn build(context: &Context) -> Result<PathBuf> {
    context.require_os(TargetOs::Macos, "pkg")?;
    let root = reset_dir(&context.work_dir.join("pkg"))?;
    let payload = root.join("payload");
    copy_binary(context, &payload.join("usr/local/bin"))?;
    write_file(
        &payload.join("Library/Application Support/vcms/config.toml.sample"),
        config_sample(),
    )?;
    write_file(
        &payload.join("Library/LaunchDaemons/com.velopulent.vcms.plist"),
        launchd_plist(),
    )?;
    write_file(&root.join("preinstall"), preinstall())?;
    write_file(&root.join("postinstall"), postinstall())?;
    let component = root.join("vcms-component.pkg");
    let artifact = context
        .out_dir
        .join(format!("vcms-{}-macos-{}.pkg", context.version, context.arch.as_str()));
    if context.dry_run {
        fs::write(&artifact, b"dry-run pkg\n")?;
    } else {
        run_cmd(
            Command::new("chmod")
                .arg("755")
                .arg(root.join("preinstall"))
                .arg(root.join("postinstall")),
        )?;
        run_cmd(
            Command::new("pkgbuild")
                .arg("--root")
                .arg(&payload)
                .arg("--scripts")
                .arg(&root)
                .arg("--identifier")
                .arg("com.velopulent.vcms")
                .arg("--version")
                .arg(&context.version)
                .arg(&component),
        )?;
        run_cmd(
            Command::new("productbuild")
                .arg("--package")
                .arg(&component)
                .arg(&artifact),
        )?;
    }
    Ok(artifact)
}

fn launchd_plist() -> &'static str {
    include_str!("../../packaging/macos/com.velopulent.vcms.plist")
}

fn preinstall() -> &'static str {
    include_str!("../../packaging/macos/preinstall")
}

fn postinstall() -> &'static str {
    include_str!("../../packaging/macos/postinstall")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_uses_dedicated_identity_and_paths() {
        let plist = launchd_plist();
        assert!(plist.contains("<key>UserName</key><string>_vcms</string>"));
        assert!(plist.contains("/Library/Application Support/vcms"));
        assert!(plist.contains("StandardErrorPath"));
    }

    #[test]
    fn lifecycle_uses_modern_launchctl() {
        assert!(preinstall().contains("launchctl bootout"));
        assert!(postinstall().contains("launchctl bootstrap"));
        assert!(!postinstall().contains("launchctl load"));
        assert!(postinstall().contains("/Library/LaunchDaemons/com.velopulent.vcms.plist"));
    }
}
