fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=proto/dummy.proto");

    tonic_build::compile_protos("proto/dummy.proto")
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
