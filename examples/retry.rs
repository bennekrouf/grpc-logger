use grpc_logger::{config::load_config, LogConfig, LoggingService};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info};

async fn init_with_retry(
    config: &LogConfig,
    service: LoggingService,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut retry_count = 0;
    let retry_config = &config.client_retry; // Using client retry settings
    let base_delay = Duration::from_secs(retry_config.base_delay_secs);

    loop {
        debug!("Attempting to initialize logging service");
        match service.init(&config).await {
            Ok(_) => {
                info!("Successfully initialized log server");
                return Ok(());
            }
            Err(e) => {
                retry_count += 1;
                if retry_count > retry_config.max_retries {
                    error!("Failed to initialize after {} retries", retry_count);
                    return Err(e.into());
                }
                let delay = base_delay.mul_f32(1.5f32.powi(retry_count as i32));
                error!(
                    "Initialization failed: {}. Retrying in {}s...",
                    e,
                    delay.as_secs()
                );
                sleep(delay).await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load configuration with retry settings
    let config = load_config("examples/retry.yaml")?;

    // Create service
    let service = LoggingService::new();

    // Initialize logging with retry mechanism
    init_with_retry(&config, service.clone()).await?;

    // Main loop with reconnection logic
    loop {
        info!("Retry example - heartbeat message");

        if let Err(e) = service.check_connection().await {
            error!("Connection lost: {}. Reconnecting...", e);
            init_with_retry(&config, service.clone()).await?;
        }

        sleep(Duration::from_secs(
            config.client_retry.reconnect_delay_secs,
        ))
        .await;
    }
}
