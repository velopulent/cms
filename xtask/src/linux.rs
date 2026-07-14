use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::{Architecture, Context, Result, TargetOs};
use crate::shared::{config_sample, copy_binary, find_file, reset_dir, run_cmd, write_file};

pub fn build_deb(context: &Context) -> Result<PathBuf> {
    context.require_os(TargetOs::Linux, "deb")?;
    let root = reset_dir(&context.work_dir.join("deb"))?;
    let package_root = root.join("root");
    let debian = package_root.join("DEBIAN");
    fs::create_dir_all(&debian)?;
    stage_payload(context, &package_root, "lib/systemd/system/vcms.service")?;
    write_file(&debian.join("control"), deb_control(context))?;
    write_file(&debian.join("preinst"), deb_preinst())?;
    write_file(&debian.join("postinst"), deb_postinst())?;
    write_file(&debian.join("prerm"), deb_prerm())?;
    write_file(&debian.join("postrm"), deb_postrm())?;
    let artifact = context
        .out_dir
        .join(format!("vcms_{}_{}.deb", context.version, deb_arch(context.arch)));
    if context.dry_run {
        fs::write(&artifact, b"dry-run deb\n")?;
    } else {
        run_cmd(Command::new("chmod").arg("755").args([
            debian.join("preinst"),
            debian.join("postinst"),
            debian.join("prerm"),
            debian.join("postrm"),
        ]))?;
        run_cmd(
            Command::new("dpkg-deb")
                .args(["--build", "--root-owner-group"])
                .arg(&package_root)
                .arg(&artifact),
        )?;
    }
    Ok(artifact)
}

pub fn build_rpm(context: &Context) -> Result<PathBuf> {
    context.require_os(TargetOs::Linux, "rpm")?;
    let root = reset_dir(&context.work_dir.join("rpm"))?;
    let rpmbuild = root.join("rpmbuild");
    for name in ["BUILD", "BUILDROOT", "RPMS", "SOURCES", "SPECS", "SRPMS"] {
        fs::create_dir_all(rpmbuild.join(name))?;
    }
    let source_root = root.join(format!("vcms-{}", context.version));
    stage_payload(context, &source_root, "usr/lib/systemd/system/vcms.service")?;
    let source_tar = rpmbuild
        .join("SOURCES")
        .join(format!("vcms-{}.tar.gz", context.version));
    if context.dry_run {
        fs::write(&source_tar, b"dry-run rpm source\n")?;
    } else {
        run_cmd(
            Command::new("tar")
                .arg("-czf")
                .arg(&source_tar)
                .arg("-C")
                .arg(&root)
                .arg(format!("vcms-{}", context.version)),
        )?;
    }
    let spec = rpmbuild.join("SPECS/vcms.spec");
    write_file(&spec, rpm_spec(context))?;
    let artifact = context
        .out_dir
        .join(format!("vcms-{}-1.{}.rpm", context.version, rpm_arch(context.arch)));
    if context.dry_run {
        fs::write(&artifact, b"dry-run rpm\n")?;
    } else {
        run_cmd(
            Command::new("rpmbuild")
                .arg("-bb")
                .arg("--define")
                .arg(format!("_topdir {}", rpmbuild.display()))
                .arg(&spec),
        )?;
        fs::copy(find_file(&rpmbuild.join("RPMS"), "rpm")?, &artifact)?;
    }
    Ok(artifact)
}

fn stage_payload(context: &Context, root: &Path, service_path: &str) -> Result<()> {
    copy_binary(context, &root.join("usr/bin"))?;
    write_file(&root.join("etc/vcms/config.toml"), config_sample())?;
    write_file(&root.join("etc/vcms/config.toml.sample"), config_sample())?;
    fs::create_dir_all(root.join("var/lib/vcms"))?;
    write_file(&root.join(service_path), systemd_service())
}

fn systemd_service() -> &'static str {
    include_str!("../../packaging/linux/vcms.service")
}

fn deb_control(context: &Context) -> String {
    include_str!("../../packaging/debian/control.template")
        .replace("@VERSION@", &context.version)
        .replace("@ARCH@", deb_arch(context.arch))
}

fn deb_preinst() -> &'static str {
    include_str!("../../packaging/debian/preinst")
}

fn deb_postinst() -> &'static str {
    include_str!("../../packaging/debian/postinst")
}

fn deb_prerm() -> &'static str {
    include_str!("../../packaging/debian/prerm")
}

fn deb_postrm() -> &'static str {
    include_str!("../../packaging/debian/postrm")
}

fn rpm_spec(context: &Context) -> String {
    include_str!("../../packaging/rpm/vcms.spec.template").replace("@VERSION@", &context.version)
}

const fn deb_arch(arch: Architecture) -> &'static str {
    match arch {
        Architecture::Amd64 => "amd64",
        Architecture::Arm64 => "arm64",
    }
}

const fn rpm_arch(arch: Architecture) -> &'static str {
    match arch {
        Architecture::Amd64 => "x86_64",
        Architecture::Arm64 => "aarch64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_is_hardened_and_uses_journal() {
        let unit = systemd_service();
        assert!(unit.contains("ProtectSystem=strict"));
        assert!(unit.contains("CapabilityBoundingSet="));
        assert!(!unit.contains("LOG_OUTPUT=file"));
    }

    #[test]
    fn first_install_does_not_start() {
        assert!(
            !deb_postinst()
                .lines()
                .any(|line| line.trim().starts_with("systemctl start"))
        );
        assert!(deb_postinst().contains("try-restart") || deb_preinst().contains("was-active"));
    }
}
