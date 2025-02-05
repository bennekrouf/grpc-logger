use crate::config::{setup_logging, LogConfig, LogOutput};
use futures::Stream;
use futures::StreamExt;
use std::pin::Pin;
use tokio::sync::broadcast;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use tracing::{info, warn};
pub mod logging {
    tonic::include_proto!("logging");
}
use logging::log_service_server::LogService;
use logging::log_service_server::LogServiceServer;
use logging::{LogMessage, SubscribeRequest};
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Server;

#[derive(Debug, Clone)]
pub struct LoggingService {
    pub sender: broadcast::Sender<LogMessage>,
    server_handle: Arc<
        Mutex<
            Option<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
        >,
    >,
}

impl LoggingService {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self {
            sender,
            server_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the entire logging service, including setting up logging and starting the server
    pub async fn init(&self, config: &LogConfig) -> Result<(), Box<dyn std::error::Error>> {
        // Setup logging first
        let service_clone = self.clone();
        let _guard = setup_logging(config, Some(service_clone))?;

        // Log initialization details
        info!("Logger initialized with output: {:?}", config.output);
        match &config.output {
            LogOutput::File => {
                info!(
                    "File logging enabled - path: {}, filename: {}",
                    config.file_path.as_deref().unwrap_or("default"),
                    config.file_name.as_deref().unwrap_or("app.log")
                );
            }
            LogOutput::Grpc => {
                if let Some(grpc_config) = &config.grpc {
                    info!(
                        "GRPC logging enabled - server running on {}:{}",
                        grpc_config.address, grpc_config.port
                    );
                }
            }
            LogOutput::Console => {
                info!("Console logging enabled");
            }
        }
        info!("Log level set to: {}", config.level);

        // Start test log generation
        let _ = self.sender.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                info!("Test log message from server");
            }
        });

        // Start the gRPC server
        self.start_server(config).await
    }

    /// Internal method to start the gRPC server
    async fn start_server(&self, config: &LogConfig) -> Result<(), Box<dyn std::error::Error>> {
        let addr = match &config.grpc {
            Some(grpc_config) => format!("{}:{}", grpc_config.address, grpc_config.port),
            None => "0.0.0.0:50052".to_string(),
        }
        .parse()?;

        let service = self.clone();
        let handle = tokio::spawn(async move {
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
                        eprintln!("Port already in use. Please stop other instances first.");
                    }
                    Err(e.into())
                }
            }
        });

        let mut server_handle = self.server_handle.lock().await;
        *server_handle = Some(handle);
        Ok(())
    }

    pub fn broadcast_log(&self, log: LogMessage) {
        if self.sender.receiver_count() > 0 {
            if let Err(e) = self.sender.send(log) {
                warn!("Failed to broadcast log: {}", e);
            }
        }
    }
}

#[tonic::async_trait]
impl LogService for LoggingService {
    type SubscribeToLogsStream = Pin<Box<dyn Stream<Item = Result<LogMessage, Status>> + Send>>;

    async fn subscribe_to_logs(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeToLogsStream>, Status> {
        println!("New client connected: {}", request.into_inner().client_id);
        let receiver = self.sender.subscribe();
        let stream = BroadcastStream::new(receiver);
        let mapped_stream = Box::pin(stream.map(|result| map_broadcast_result(result)));
        Ok(Response::new(mapped_stream))
    }
}

fn map_broadcast_result(
    result: Result<LogMessage, BroadcastStreamRecvError>,
) -> Result<LogMessage, Status> {
    result.map_err(|e| Status::internal(format!("Failed to receive log message: {}", e)))
}
