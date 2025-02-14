use crate::server_build::logging::log_service_client::LogServiceClient;
use crate::server_build::logging::SubscribeRequest;
use tonic::transport::Channel;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct GrpcLoggerClient {
    _client: Arc<tokio::sync::Mutex<LogServiceClient<Channel>>>,
}

impl GrpcLoggerClient {
    pub async fn new(addr: String) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        println!("GRPC Logger: Creating new client connection to {}", addr);
        let client = LogServiceClient::connect(addr).await?;

        // Keep the client active by subscribing but not processing messages
        let mut client_clone = client.clone();
        tokio::spawn(async move {
            let request = tonic::Request::new(SubscribeRequest {
                client_id: "logger".to_string(),
            });

            println!("GRPC Logger: Starting log subscription");
            match client_clone.subscribe_to_logs(request).await {
                Ok(response) => {
                    println!("GRPC Logger: Successfully subscribed to logs");
                    let mut stream = response.into_inner();
                    // Just keep the connection alive by receiving messages
                    while let Ok(Some(_)) = stream.message().await {
                        sleep(Duration::from_secs(1)).await; // Prevent busy-loop
                    }
                }
                Err(e) => {
                    println!("GRPC Logger Error: Failed to establish log stream: {}", e);
                }
            }
        });

        println!("GRPC Logger: Client setup complete");
        Ok(Self {
            _client: Arc::new(tokio::sync::Mutex::new(client))
        })
    }
}
