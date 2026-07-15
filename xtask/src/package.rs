use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::artifact;
use crate::model::{Architecture, Context, PackageKind, Result, TargetOs};
use crate::{linux, macos, portable, shared, windows};

#[derive(Debug, Clone)]
pub struct PackageRequest {
    pub kind: PackageKind,
    pub version: String,
    pub target_os: TargetOs,
    pub arch: Architecture,
    pub dry_run: bool,
    pub skip_build: bool,
}

pub fn run(request: PackageRequest) -> Result<()> {
    let root = repo_root()?;
    let out_dir = root.join("dist/packages");
    let work_dir = root.join("target/package");
    fs::create_dir_all(&out_dir)?;
    fs::create_dir_all(&work_dir)?;

    if !request.skip_build && !request.dry_run {
        shared::run_cmd(Command::new("bun").args(["run", "build:dashboard"]).current_dir(&root))?;
        shared::run_cmd(
            Command::new("cargo")
                .args(["build", "--release", "--locked"])
                .current_dir(root.join("apps/backend"))
                .env("SKIP_DASHBOARD_BUILD", "1"),
        )?;
    }

    let bin_name = if request.target_os == TargetOs::Windows {
        "vcms.exe"
    } else {
        "vcms"
    }
    .to_owned();
    let binary = root.join("target/release").join(&bin_name);
    if !request.dry_run && !binary.is_file() {
        return Err(format!("release binary missing at {}", binary.display()).into());
    }
    let context = Context {
        out_dir,
        work_dir,
        version: request.version,
        target_os: request.target_os,
        arch: request.arch,
        bin_name,
        binary,
        dry_run: request.dry_run,
    };
    let artifacts = build(&context, request.kind)?;
    let metadata = artifact::write_metadata(&context, &artifacts)?;
    for path in artifacts.into_iter().chain(metadata) {
        println!("{}", path.display());
    }
    Ok(())
}

fn build(context: &Context, kind: PackageKind) -> Result<Vec<PathBuf>> {
    match kind {
        PackageKind::Portable => Ok(vec![portable::build(context)?]),
        PackageKind::Deb => Ok(vec![linux::build_deb(context)?]),
        PackageKind::Rpm => Ok(vec![linux::build_rpm(context)?]),
        PackageKind::Msi => Ok(vec![windows::build(context)?]),
        PackageKind::Pkg => Ok(vec![macos::build(context)?]),
        PackageKind::Host | PackageKind::All => {
            let mut output = vec![portable::build(context)?];
            match context.target_os {
                TargetOs::Linux => {
                    output.push(linux::build_deb(context)?);
                    output.push(linux::build_rpm(context)?);
                }
                TargetOs::Macos => output.push(macos::build(context)?),
                TargetOs::Windows => output.push(windows::build(context)?),
            }
            Ok(output)
        }
    }
}

pub fn repo_root() -> Result<PathBuf> {
    let mut directory = env::current_dir()?;
    loop {
        if directory.join("Cargo.toml").is_file() && directory.join("apps/backend/Cargo.toml").is_file() {
            return Ok(directory);
        }
        if !directory.pop() {
            return Err("could not find repo root".into());
        }
    }
}
