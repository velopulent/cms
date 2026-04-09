use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=dashboard/");
    println!("cargo:rerun-if-changed=proto/");

    let status = Command::new("bun")
        .arg("run")
        .arg("build")
        .current_dir("dashboard")
        .status()
        .expect("Failed to build dashboard");

    if !status.success() {
        panic!("Dashboard build failed");
    }

    let proto_files = ["proto/cms.proto"];
    let include_dirs = ["proto"];
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .out_dir("src/grpc/cms")
        .compile_protos(&proto_files, &include_dirs)
        .expect("Failed to compile proto");
}
