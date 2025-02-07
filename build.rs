use std::env;
use std::path::PathBuf;

fn main() {
    // Get the OUT_DIR environment variable at runtime
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Construct the path to the descriptor set file
    let descriptor_path = out_dir.join("logging_descriptor.bin");

    // Configure and compile the proto files
    tonic_build::configure()
        .file_descriptor_set_path(descriptor_path)
        .compile_protos(&["log_service.proto"], &["proto"])
        .unwrap_or_else(|e| panic!("Failed to compile proto files: {}", e));
}

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//
//     let proto_file = "./log_service";
//     let descriptor_path = Path::new(&env::var("OUT_DIR")?).join("logging_descriptor.bin");
//     tonic_build::configure()
//         .file_descriptor_set_path(&descriptor_path)  // This generates the descriptor
//         .compile(&[proto_file], &["proto"])?;
//     Ok(())
// }
