use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub const APP_NAME: &str = "vcms";
pub const PACKAGE_NAME: &str = "vcms";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Linux,
    Macos,
    Windows,
}

impl TargetOs {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "linux" => Ok(Self::Linux),
            "macos" | "darwin" => Ok(Self::Macos),
            "windows" => Ok(Self::Windows),
            other => Err(format!("unsupported target OS '{other}'").into()),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    Amd64,
    Arm64,
}

impl Architecture {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "amd64" | "x86_64" | "x64" => Ok(Self::Amd64),
            "arm64" | "aarch64" => Ok(Self::Arm64),
            other => Err(format!("unsupported architecture '{other}'").into()),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Host,
    Portable,
    Deb,
    Rpm,
    Msi,
    Pkg,
    All,
}

impl PackageKind {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "host" => Ok(Self::Host),
            "portable" => Ok(Self::Portable),
            "deb" => Ok(Self::Deb),
            "rpm" => Ok(Self::Rpm),
            "msi" => Ok(Self::Msi),
            "pkg" | "macos-pkg" => Ok(Self::Pkg),
            "all" => Ok(Self::All),
            other => Err(format!("unknown package kind '{other}'").into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    pub out_dir: PathBuf,
    pub work_dir: PathBuf,
    pub version: String,
    pub target_os: TargetOs,
    pub arch: Architecture,
    pub bin_name: String,
    pub binary: PathBuf,
    pub dry_run: bool,
}

impl Context {
    pub fn require_os(&self, expected: TargetOs, kind: &str) -> Result<()> {
        if self.target_os == expected || self.dry_run {
            Ok(())
        } else {
            Err(format!("{kind} packages must be built on --target-os {}", expected.as_str()).into())
        }
    }
}
