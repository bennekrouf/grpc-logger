pub mod config;
pub mod grpc;
pub mod server_build;
pub mod setup_logging;

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub client_id: String,
    pub client_type: i32,
    pub server_name: String,
}

pub use crate::server_build::logging::ClientType;

pub use crate::server_build::LoggingService;
pub use grpc::GrpcLayer;
pub use config::{load_config, LogConfig};
pub use setup_logging::setup_logging;

/// Initialize the logging service with a given configuration file
pub async fn init(config_path: &str) -> Result<LoggingService, Box<dyn std::error::Error + Send + Sync>> {
    let config = load_config(config_path)?;
    let service = LoggingService::new();

    // Initialize the service
    service.init(&config).await?;

    Ok(service)
}
