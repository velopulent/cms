use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::model::{APP_NAME, Context, Result, TargetOs};
use crate::shared::{copy_binary, reset_dir, run_cmd, write_file};

pub fn build(context: &Context) -> Result<PathBuf> {
    let name = format!(
        "vcms-{}-{}-{}",
        context.version,
        context.target_os.as_str(),
        context.arch.as_str()
    );
    let stage = reset_dir(&context.work_dir.join(&name))?;
    copy_binary(context, &stage)?;
    write_file(
        &stage.join("README.txt"),
        format!(
            "Velopulent CMS portable package\n\nRun `{APP_NAME}` directly. No service is registered. Runtime files use platform user directories unless VCMS_HOME is set.\nTarget OS: {}\n",
            context.target_os.as_str()
        ),
    )?;
    if context.target_os == TargetOs::Windows {
        let artifact = context.out_dir.join(format!("{name}.exe"));
        if context.dry_run {
            fs::write(&artifact, b"dry-run portable exe\n")?;
        } else {
            fs::copy(&context.binary, &artifact)?;
        }
        Ok(artifact)
    } else {
        let artifact = context.out_dir.join(format!("{name}.tar.gz"));
        if context.dry_run {
            fs::write(&artifact, b"dry-run portable tarball\n")?;
        } else {
            run_cmd(
                Command::new("tar")
                    .args(["-czf"])
                    .arg(&artifact)
                    .arg("-C")
                    .arg(&context.work_dir)
                    .arg(&name),
            )?;
        }
        Ok(artifact)
    }
}
