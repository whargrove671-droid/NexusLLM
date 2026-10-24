fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the protoc we installed
    let protoc_path = "C:\\Users\\Diamond\\.cargo\\bin\\protoc.exe";
    std::env::set_var("PROTOC", protoc_path);
    
    prost_build::Config::new()
        .compile_protos(
            &["../../proto/nexus.proto"],
            &["../../proto"],
        )?;
    Ok(())
}