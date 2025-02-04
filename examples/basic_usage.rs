use grpc_logger::{LoggingService, config::{load_config, setup_logging}};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config("config.yaml")?;
    let service = LoggingService::new();

    let _guard = setup_logging(&config, Some(service.clone()))?;

    info!("This is a test log message");

    // Keep running for a while
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

    Ok(())
}
