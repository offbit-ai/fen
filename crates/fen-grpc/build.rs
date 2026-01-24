use std::io::Result;

fn main() -> Result<()> {
    // Tell Cargo to rerun if proto files change
    println!("cargo:rerun-if-changed=proto/shard_service.proto");

    // Compile the proto files with tonic
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/shard_service.proto"], &["proto/"])?;

    Ok(())
}
