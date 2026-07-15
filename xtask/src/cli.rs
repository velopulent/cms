use std::env;
use std::fs;
use std::path::PathBuf;

use crate::arch;
use crate::model::{Architecture, PackageKind, Result, TargetOs};
use crate::package::{self, PackageRequest};

pub fn run() -> Result<()> {
    let mut raw = env::args().skip(1);
    let command = raw.next().unwrap_or_else(|| "help".to_owned());
    if command == "arch-render" {
        return render_arch(raw);
    }
    let args = Args::parse(std::iter::once(command).chain(raw))?;
    match args.command.as_str() {
        "package" => package::run(args.into()),
        "package-dry-run" => package::run(PackageRequest {
            dry_run: true,
            ..args.into()
        }),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown xtask command '{other}'").into()),
    }
}

fn render_arch<I>(mut raw: I) -> Result<()>
where
    I: Iterator<Item = String>,
{
    let mut version = None;
    let mut amd64 = None;
    let mut arm64 = None;
    let mut output = PathBuf::from("dist/arch");
    while let Some(arg) = raw.next() {
        match arg.as_str() {
            "--version" => version = Some(need_value("--version", raw.next())?),
            "--amd64" => amd64 = Some(PathBuf::from(need_value("--amd64", raw.next())?)),
            "--arm64" => arm64 = Some(PathBuf::from(need_value("--arm64", raw.next())?)),
            "--out" => output = PathBuf::from(need_value("--out", raw.next())?),
            other => return Err(format!("unknown arch-render argument '{other}'").into()),
        }
    }
    let files = arch::render(
        version.as_deref().ok_or("arch-render needs --version")?,
        amd64.as_deref().ok_or("arch-render needs --amd64")?,
        arm64.as_deref().ok_or("arch-render needs --arm64")?,
        &output,
    )?;
    for file in files {
        println!("{}", file.display());
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct Args {
    command: String,
    kind: PackageKind,
    version: String,
    target_os: TargetOs,
    arch: Architecture,
    dry_run: bool,
    skip_build: bool,
}

impl Args {
    fn parse<I>(mut raw: I) -> Result<Self>
    where
        I: Iterator<Item = String>,
    {
        let command = raw.next().unwrap_or_else(|| "help".to_owned());
        let mut output = Self {
            command,
            kind: PackageKind::Host,
            version: match env::var("GITHUB_REF_NAME").ok() {
                Some(value) => value.strip_prefix('v').unwrap_or(&value).to_owned(),
                None => backend_version()?,
            },
            target_os: TargetOs::parse(env::consts::OS)?,
            arch: Architecture::parse(env::consts::ARCH)?,
            dry_run: false,
            skip_build: false,
        };
        while let Some(arg) = raw.next() {
            match arg.as_str() {
                "--kind" => output.kind = PackageKind::parse(&need_value("--kind", raw.next())?)?,
                "--version" => output.version = need_value("--version", raw.next())?,
                "--target-os" => output.target_os = TargetOs::parse(&need_value("--target-os", raw.next())?)?,
                "--arch" => output.arch = Architecture::parse(&need_value("--arch", raw.next())?)?,
                "--dry-run" => output.dry_run = true,
                "--skip-build" => output.skip_build = true,
                "-h" | "--help" => output.command = "help".to_owned(),
                other => return Err(format!("unknown package argument '{other}'").into()),
            }
        }
        Ok(output)
    }
}

impl From<Args> for PackageRequest {
    fn from(value: Args) -> Self {
        Self {
            kind: value.kind,
            version: value.version,
            target_os: value.target_os,
            arch: value.arch,
            dry_run: value.dry_run,
            skip_build: value.skip_build,
        }
    }
}

fn need_value(name: &str, value: Option<String>) -> Result<String> {
    value.ok_or_else(|| format!("{name} needs a value").into())
}

fn backend_version() -> Result<String> {
    let root = package::repo_root()?;
    let cargo = fs::read_to_string(root.join("apps/backend/Cargo.toml"))?;
    cargo
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("version = ")
                .map(|value| value.trim_matches('"').to_owned())
        })
        .ok_or_else(|| "apps/backend/Cargo.toml has no version".into())
}

fn print_help() {
    println!(
        "usage:\n  cargo run -p xtask -- package [--kind host|portable|deb|rpm|msi|pkg|all] [--version X] [--target-os linux|macos|windows] [--arch amd64|arm64] [--dry-run] [--skip-build]\n  cargo run -p xtask -- arch-render --version X --amd64 ARCHIVE --arm64 ARCHIVE [--out DIR]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_target_aliases() {
        let args = Args::parse(
            ["package", "--target-os", "darwin", "--arch", "aarch64", "--dry-run"]
                .into_iter()
                .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(args.target_os, TargetOs::Macos);
        assert_eq!(args.arch, Architecture::Arm64);
        assert!(args.dry_run);
    }
}
