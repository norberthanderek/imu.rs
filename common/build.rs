fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/");
    prost_build::Config::new().compile_protos(&["proto/imu.proto"], &["proto/"])?;
    Ok(())
}
