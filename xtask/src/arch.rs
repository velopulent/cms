use std::fs;
use std::path::{Path, PathBuf};

use crate::artifact::sha256_file;
use crate::model::Result;
use crate::shared::write_file;

const TEMPLATE: &str = include_str!("../../packaging/arch/PKGBUILD.template");
const SERVICE: &str = include_str!("../../packaging/linux/vcms.service");

pub fn render(version: &str, amd64: &Path, arm64: &Path, output: &Path) -> Result<Vec<PathBuf>> {
    if !amd64.is_file() || !arm64.is_file() {
        return Err("both Linux release archives must exist before rendering the Arch recipe".into());
    }
    fs::create_dir_all(output)?;
    let service_path = output.join("vcms.service");
    write_file(&service_path, SERVICE)?;
    let pkgbuild = TEMPLATE
        .replace("@VERSION@", version)
        .replace("@SHA256_AMD64@", &sha256_file(amd64)?)
        .replace("@SHA256_ARM64@", &sha256_file(arm64)?)
        .replace("@SHA256_SERVICE@", &sha256_file(&service_path)?);
    let pkgbuild_path = output.join("PKGBUILD");
    write_file(&pkgbuild_path, pkgbuild)?;
    Ok(vec![pkgbuild_path, service_path])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_complete_recipe_with_real_hashes() {
        let root = std::env::temp_dir().join(format!("vcms-arch-render-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let amd64 = root.join("amd64.tar.gz");
        let arm64 = root.join("arm64.tar.gz");
        fs::write(&amd64, b"amd64").unwrap();
        fs::write(&arm64, b"arm64").unwrap();
        let output = root.join("out");
        render("1.2.3", &amd64, &arm64, &output).unwrap();
        let recipe = fs::read_to_string(output.join("PKGBUILD")).unwrap();
        assert!(recipe.contains("pkgver=1.2.3"));
        assert!(!recipe.contains('@'));
        assert!(!recipe.contains("'SKIP'"));
        fs::remove_dir_all(root).unwrap();
    }
}
