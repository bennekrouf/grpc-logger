mod server_build;
mod grpc;
mod config;
use grpc_logger::init;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize service with config file
    let _service = init("config.yaml").await?;

    // Keep the main task running until ctrl-c
    tokio::signal::ctrl_c().await?;
    println!("Shutting down server...");

    Ok(())
}


