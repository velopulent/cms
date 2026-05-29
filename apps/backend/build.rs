use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let manifest_path = PathBuf::from(manifest_dir);
    let workspace_root = manifest_path
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to resolve workspace root from CARGO_MANIFEST_DIR");

    let proto_dir = workspace_root.join("libs/proto");
    let proto_file = proto_dir.join("cms.proto");

    let proto_dir_str = proto_dir.to_str().expect("Invalid proto dir path");
    let proto_file_str = proto_file.to_str().expect("Invalid proto file path");

    println!("cargo:rerun-if-changed={}", proto_dir_str);

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/grpc/cms")
        .file_descriptor_set_path(
            PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("cms_descriptor.bin"),
        )
        .compile_protos(&[proto_file_str], &[proto_dir_str])
        .expect("Failed to compile proto");
}
