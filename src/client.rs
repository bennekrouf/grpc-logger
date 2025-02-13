use tonic::transport::Channel;
use crate::server_build::logging::log_service_client::LogServiceClient;
use crate::server_build::logging::LogMessage;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use std::sync::Arc;

pub struct GrpcLoggerClient {
    client: Arc<tokio::sync::Mutex<LogServiceClient<Channel>>>,
    server_id: Option<String>,
}

impl GrpcLoggerClient {
    pub async fn new(addr: String, server_id: Option<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = LogServiceClient::connect(addr).await?;
        Ok(Self {
            client: Arc::new(tokio::sync::Mutex::new(client)),
            server_id,
        })
    }

    pub async fn send_log(&self, message: LogMessage) {
        // Implementation for sending logs
    }
}

pub struct GrpcClientLayer {
    client: Arc<GrpcLoggerClient>,
}

impl GrpcClientLayer {
    pub fn new(client: GrpcLoggerClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

impl<S> Layer<S> for GrpcClientLayer
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let client = self.client.clone();
        let message = LogMessage {
            timestamp: Some(chrono::Local::now().to_rfc3339()),
            level: Some(event.metadata().level().to_string()),
            message: format!("{:?}", event),
            server_id: self.client.server_id.clone(),
            target: Some(event.metadata().target().to_string()),
            // ... fill other fields as needed
            ..Default::default()
        };

        tokio::spawn(async move {
            client.send_log(message).await;
        });
    }
}
