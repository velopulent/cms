use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::{Context, PACKAGE_NAME, Result};

pub fn write_metadata(context: &Context, artifacts: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let stem = format!(
        "vcms-{}-{}-{}",
        context.version,
        context.target_os.as_str(),
        context.arch.as_str()
    );
    let manifest = context.out_dir.join(format!("{stem}-manifest.json"));
    let checksums = context.out_dir.join(format!("{stem}-SHA256SUMS"));

    let entries = artifacts
        .iter()
        .map(|artifact| {
            let name = artifact.file_name().unwrap_or_default().to_string_lossy();
            Ok(format!(
                "    {{\"name\":\"{}\",\"sha256\":\"{}\"}}",
                name,
                sha256_file(artifact)?
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    fs::write(
        &manifest,
        format!(
            "{{\n  \"schema_version\": 1,\n  \"name\": \"{PACKAGE_NAME}\",\n  \"version\": \"{}\",\n  \"target_os\": \"{}\",\n  \"arch\": \"{}\",\n  \"artifacts\": [\n{}\n  ]\n}}\n",
            context.version,
            context.target_os.as_str(),
            context.arch.as_str(),
            entries.join(",\n")
        ),
    )?;
    let mut checksum_file = fs::File::create(&checksums)?;
    for artifact in artifacts {
        writeln!(
            checksum_file,
            "{}  {}",
            sha256_file(artifact)?,
            artifact.file_name().unwrap_or_default().to_string_lossy()
        )?;
    }
    Ok(vec![manifest, checksums])
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut output = String::with_capacity(64);
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}")?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_stable() {
        let path = std::env::temp_dir().join(format!("vcms-xtask-sha-{}", std::process::id()));
        fs::write(&path, b"vcms").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "fac1f37320e181f97fa88d454816f82ab8fefcc875d338a57bf9f4a974c9ffb7"
        );
        fs::remove_file(path).unwrap();
    }
}
