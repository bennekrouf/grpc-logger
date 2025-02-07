use crate::config::{setup_logging, LogConfig, LogOutput};
use futures::Stream;
use futures::future;
use std::time::Duration;
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
use tonic_reflection::server::Builder;
use tower_http::cors::{Any, CorsLayer};

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

    let descriptor_set = include_bytes!(concat!(env!("OUT_DIR"), "/logging_descriptor.bin"));
    let reflection_service = Builder::configure()
    .register_encoded_file_descriptor_set(descriptor_set)
    .build_v1()?;

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any)
        .expose_headers(Any);

    let service = self.clone();
    let handle = tokio::spawn(async move {
        match Server::builder()
            .accept_http1(true)
            .max_concurrent_streams(128)  // Set reasonable limits
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .tcp_nodelay(true)
            .layer(cors)  // Add CORS layer
            .layer(GrpcWebLayer::new())
            .add_service(LogServiceServer::new(service))
            .add_service(reflection_service)  // Add reflection service
            // .serve(addr)
            .serve_with_shutdown(addr, async {
                tokio::signal::ctrl_c().await.ok();
                println!("Shutting down server...");
            })
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
        
        // Create a stream that handles end of stream properly
        let stream = BroadcastStream::new(receiver)
            .map(|result| map_broadcast_result(result))
            .take_while(|result| {
                // Continue streaming unless we get an error
                future::ready(result.is_ok())
            });

        let mapped_stream = Box::pin(stream);
        Ok(Response::new(mapped_stream))
    }
}

fn map_broadcast_result(
    result: Result<LogMessage, BroadcastStreamRecvError>,
) -> Result<LogMessage, Status> {
    match result {
        Ok(msg) => Ok(msg),
        Err(BroadcastStreamRecvError::Lagged(n)) => {
            println!("Client lagging behind by {} messages", n);
            Err(Status::resource_exhausted(format!(
                "Client lagging behind by {} messages",
                n
            )))
        }
    }
}
