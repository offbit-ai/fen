use std::io::Result;

fn main() -> Result<()> {
    // Tell Cargo to rerun if proto files change
    println!("cargo:rerun-if-changed=proto/events.proto");

    // Configure prost-build
    let mut config = prost_build::Config::new();

    // Compile the proto files
    // Note: We don't add serde derives globally because prost_types::Timestamp
    // doesn't implement serde traits. JSON serialization should use the
    // wrapper types in the events module or convert to domain types.
    config.compile_protos(&["proto/events.proto"], &["proto/"])?;

    Ok(())
}
