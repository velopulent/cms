use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=ui/");

    let status = Command::new("bun")
        .arg("run")
        .arg("build")
        .current_dir("ui")
        .status()
        .expect("Failed to build frontend");

    if !status.success() {
        panic!("Frontend build failed");
    }
}
