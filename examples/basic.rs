use grpc_logger::{config::load_config, LoggingService};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load basic configuration
    let config = load_config("examples/basic.yaml")?;

    // Create and initialize logging service
    let service = LoggingService::new();
    service.init(&config).await?;

    // Send some test logs
    info!("Basic example - test message 1");
    info!("Basic example - test message 2");

    // Keep the connection alive for a few seconds
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    Ok(())
}
