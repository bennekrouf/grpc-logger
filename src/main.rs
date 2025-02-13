mod config;
mod grpc;
mod server_build;
mod client;

use grpc_logger::init;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    // Initialize service with config file
    let _service = init("config.yaml").await?;

    // Keep the main task running until ctrl-c
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down server...");

    Ok(())
}
