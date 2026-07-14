use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};
use uuid::Uuid;

use crate::model::{Architecture, Context, Result, TargetOs};
use crate::shared::{config_sample, copy_binary, reset_dir, run_cmd, write_file};

pub fn build(context: &Context) -> Result<PathBuf> {
    context.require_os(TargetOs::Windows, "msi")?;
    let root = reset_dir(&context.work_dir.join("msi"))?;
    let payload = root.join("payload");
    copy_binary(context, &payload)?;
    write_file(&payload.join("config.sample.toml"), config_sample())?;
    write_file(&root.join("license.rtf"), license_rtf())?;
    write_file(&root.join("vcms.wxs"), wix_source(context))?;
    let artifact = context.out_dir.join(format!(
        "vcms-{}-windows-{}.msi",
        context.version,
        context.arch.as_str()
    ));
    remove_legacy_outputs(context, &artifact)?;
    if context.dry_run {
        fs::write(&artifact, b"dry-run msi\n")?;
    } else {
        let package = root.join("vcms.msi");
        let ui_extension = wix_ui_extension()?;
        run_cmd(
            Command::new("wix")
                .arg("build")
                .args(["-acceptEula", "wix7"])
                .args(["-arch", wix_arch(context.arch)])
                .args(["-pdbtype", "none"])
                .arg("-ext")
                .arg(ui_extension)
                .arg(root.join("vcms.wxs"))
                .arg("-out")
                .arg(&package)
                // WiX resolves the relative `payload\\...` paths in the source
                // template from its working directory, not from the `.wxs` path.
                .current_dir(&root),
        )?;
        fs::copy(package, &artifact)?;
    }
    Ok(artifact)
}

fn wix_ui_extension() -> Result<PathBuf> {
    let profile = env::var_os("USERPROFILE").ok_or("USERPROFILE is not set")?;
    let extension =
        PathBuf::from(profile).join(".wix/extensions/WixToolset.UI.wixext/7.0.0/wixext7/WixToolset.UI.wixext.dll");
    if extension.is_file() {
        Ok(extension)
    } else {
        Err("WiX UI extension missing; run `wix extension add --global WixToolset.UI.wixext/7.0.0`".into())
    }
}

fn remove_legacy_outputs(context: &Context, artifact: &Path) -> Result<()> {
    // Older builds wrote WiX's external cabinet and debug symbols directly into
    // dist/packages. They are not release artifacts and can break an MSI-only upload.
    for path in [context.out_dir.join("cab1.cab"), artifact.with_extension("wixpdb")] {
        if path.is_file() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn wix_arch(arch: Architecture) -> &'static str {
    match arch {
        Architecture::Amd64 => "x64",
        Architecture::Arm64 => "arm64",
    }
}

fn wix_source(context: &Context) -> String {
    include_str!("../../packaging/windows/vcms.wxs.template")
        .replace("@VERSION@", &context.version)
        .replace("@PRODUCT_CODE@", &product_code(context))
}

fn product_code(context: &Context) -> String {
    const PRODUCT_NAMESPACE: Uuid = Uuid::from_u128(0x2c22e086_4486_46b5_9760_cbd71cb7adf0);
    let identity = format!("vcms-msi:{}:{}", context.version, context.arch.as_str());
    Uuid::new_v5(&PRODUCT_NAMESPACE, identity.as_bytes())
        .hyphenated()
        .to_string()
}

fn license_rtf() -> String {
    let mut rtf = String::from(r#"{\rtf1\ansi\deff0{\fonttbl{\f0 Segoe UI;}}\fs18 "#);
    for character in include_str!("../../LICENSE").chars() {
        match character {
            '\\' | '{' | '}' => {
                rtf.push('\\');
                rtf.push(character);
            }
            '\n' => rtf.push_str("\\line\n"),
            '\r' => {}
            _ => rtf.push(character),
        }
    }
    rtf.push('}');
    rtf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Architecture, TargetOs};

    #[test]
    fn msi_starts_service_automatically_and_uses_least_privilege() {
        let context = Context {
            out_dir: PathBuf::new(),
            work_dir: PathBuf::new(),
            version: "1.2.3".into(),
            target_os: TargetOs::Windows,
            arch: Architecture::Amd64,
            bin_name: "vcms.exe".into(),
            binary: PathBuf::new(),
            dry_run: true,
        };
        let source = wix_source(&context);
        assert!(source.contains(r#"Start="auto""#));
        assert!(source.contains(r#"<ServiceControl Id="VcmsServiceControl" Name="vcms" Start="install""#));
        assert!(source.contains(r#"Account="NT AUTHORITY\LocalService""#));
        assert!(!source.contains("LocalSystem"));
        assert!(source.contains(
            r#"<Environment Id="VcmsPath" Name="PATH" Value="[INSTALLFOLDER]" Action="set" Part="last" System="yes" Permanent="no" />"#
        ));
        assert!(source.contains("ProgramDataVcms"));
        assert!(source.contains(r#"<MediaTemplate EmbedCab="yes" />"#));
        assert!(source.contains(r#"ProductCode=""#));
        assert!(!source.contains("@PRODUCT_CODE@"));
        assert!(source.contains(r#"<ui:WixUI Id="WixUI_InstallDir""#));
        assert!(source.contains(r#"<Property Id="MsiLogging""#));
        assert!(license_rtf().starts_with(r#"{\rtf1"#));
        assert!(license_rtf().contains("GNU AFFERO GENERAL PUBLIC LICENSE"));
        assert_eq!(wix_arch(Architecture::Amd64), "x64");
        assert_eq!(wix_arch(Architecture::Arm64), "arm64");
        assert_eq!(product_code(&context), product_code(&context));

        let arm_context = Context {
            arch: Architecture::Arm64,
            ..context
        };
        assert_ne!(
            product_code(&arm_context),
            product_code(&Context {
                arch: Architecture::Amd64,
                ..arm_context.clone()
            })
        );
    }
}
