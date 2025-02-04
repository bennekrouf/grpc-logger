mod server_build;
mod grpc;
mod config;

use server_build::logging::log_service_server::LogServiceServer;
use crate::config::{setup_logging, load_config};
use serde::Deserialize;
use tonic::transport::Server;
use tonic_web::GrpcWebLayer;
use server_build::LoggingService;
use tracing::info;

// Configuration structs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LogOutput {
    Console,
    File,
    Grpc,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config("config.yaml")?;

    let service = LoggingService::new();
    let service_clone = service.clone();

    let _guard = setup_logging(&config, Some(service_clone.clone()));

    // Spawn a task that generates logs every 10 seconds
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            info!("Test log message from server");
        }
    });

    let addr = match &config.grpc {
        Some(grpc_config) => format!("{}:{}", grpc_config.address, grpc_config.port),
        None => "0.0.0.0:50052".to_string(), // fallback address
    }.parse()?;

    println!("Starting Log Server on {}", addr);
    match Server::builder()
        .accept_http1(true)
        .layer(GrpcWebLayer::new())
        .add_service(LogServiceServer::new(service))
        .serve(addr)
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            if e.to_string().contains("Address already in use") {
                eprintln!("Port {} is already in use. Please stop other instances first.", 
                    config.grpc.as_ref().map(|g| g.port).unwrap_or(0));
            }
            Err(e.into())
        }
    }
}
