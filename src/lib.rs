pub mod grpc;
pub mod config;
pub mod server_build;

pub use crate::server_build::LoggingService;
pub use grpc::GrpcLayer;
pub use config::{LogConfig, load_config, setup_logging};

/// Initialize the logging service with a given configuration file
pub async fn init(config_path: &str) -> Result<LoggingService, Box<dyn std::error::Error>> {
    let config = load_config(config_path)?;
    let service = LoggingService::new();
    
    // Initialize the service
    service.init(&config).await?;
    
    Ok(service)
}
