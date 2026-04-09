use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=dashboard/");

    let status = Command::new("bun")
        .arg("run")
        .arg("build")
        .current_dir("dashboard")
        .status()
        .expect("Failed to build dashboard");

    if !status.success() {
        panic!("Dashboard build failed");
    }
}
