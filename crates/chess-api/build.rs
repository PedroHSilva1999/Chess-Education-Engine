fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // SAFETY: build scripts run as isolated processes before compilation starts.
    unsafe { std::env::set_var("PROTOC", protoc) };
    tonic_prost_build::configure()
        .compile_protos(&["../../proto/chess.proto"], &["../../proto"])?;
    println!("cargo:rerun-if-changed=../../proto/chess.proto");
    Ok(())
}
