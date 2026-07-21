use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::{Context, Result};

pub fn run_cmd(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed with {status:?}: {command:?}").into())
    }
}

pub fn reset_dir(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(path.to_path_buf())
}

pub fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

pub fn copy_binary(ctx: &Context, directory: &Path) -> Result<()> {
    fs::create_dir_all(directory)?;
    let destination = directory.join(&ctx.bin_name);
    if ctx.dry_run {
        fs::write(destination, b"dry-run vcms binary\n")?;
    } else {
        fs::copy(&ctx.binary, destination)?;
    }
    Ok(())
}

pub fn find_file(root: &Path, extension: &str) -> Result<PathBuf> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            if let Ok(found) = find_file(&path, extension) {
                return Ok(found);
            }
        } else if path.extension() == Some(OsStr::new(extension)) {
            return Ok(path);
        }
    }
    Err(format!("no .{extension} found in {}", root.display()).into())
}

pub fn config_sample() -> &'static str {
    r#"[server]
http_address = "127.0.0.1:3000"
grpc_address = "127.0.0.1:50051"

[log]
level = "cms=info,vcms=info"
output = "file"
"#
}
