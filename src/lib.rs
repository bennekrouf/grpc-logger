pub mod config;
pub mod grpc;
pub mod server_build;
pub mod client;

pub use crate::server_build::LoggingService;
pub use grpc::GrpcLayer;
pub use config::{load_config, setup_logging, setup_client_logging, LogConfig};

/// Initialize the logging service with a given configuration file
pub async fn init(config_path: &str) -> Result<LoggingService, Box<dyn std::error::Error + Send + Sync>> {
    let config = load_config(config_path)?;
    let service = LoggingService::new();

    // Initialize the service
    service.init(&config).await?;

    Ok(service)
}
