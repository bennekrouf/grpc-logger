use crate::config::{setup_logging, LogConfig, LogOutput};
use futures::Stream;
use futures::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use tracing::{info, trace, warn};
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
    clients: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<LogMessage>>>>,
    server_handle: Arc<
        Mutex<
            Option<tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>,
        >,
    >,
    log_all_messages: Arc<Mutex<bool>>,
}

impl Default for LoggingService {
    fn default() -> Self {
        Self::new()
    }
}

impl LoggingService {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            server_handle: Arc::new(Mutex::new(None)),
            log_all_messages: Arc::new(Mutex::new(false)), // Default to false
        }
    }

    /// Initialize the entire logging service, including setting up logging and starting the server
    pub async fn init(
        &self,
        config: &LogConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Set log_all_messages from config
        {
            let mut log_all = self.log_all_messages.lock().await;
            *log_all = config.log_all_messages;
        }
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
        // let service_clone1 = self.clone();

        // Start test log generation only if debug mode is enabled
        if config.debug_mode.enabled {
            let interval_secs = config.debug_mode.test_interval_secs.max(1); // Ensure at least 1 second
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
                loop {
                    interval.tick().await;
                    trace!("Test log message from server");
                }
            });

            info!(
                "Debug mode enabled: sending test messages every {} seconds",
                interval_secs
            );
        }

        // Start the gRPC server
        self.start_server(config).await
    }

    /// Internal method to start the gRPC server
    async fn start_server(
        &self,
        config: &LogConfig,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
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
                .max_concurrent_streams(128) // Set reasonable limits
                .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
                .tcp_nodelay(true)
                .layer(cors) // Add CORS layer
                .layer(GrpcWebLayer::new())
                .add_service(LogServiceServer::new(service))
                .add_service(reflection_service) // Add reflection service
                // .serve(addr)
                .serve_with_shutdown(addr, async {
                    tokio::signal::ctrl_c().await.ok();
                    info!("Shutting down server...");
                })
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => {
                    if e.to_string().contains("Address already in use") {
                        tracing::error!("Port already in use. Please stop other instances first.");
                    }
                    Err(e.into())
                }
            }
        });

        let mut server_handle = self.server_handle.lock().await;
        *server_handle = Some(handle);
        Ok(())
    }

    pub async fn broadcast_log_filtered(&self, log: LogMessage, client_id: String) {
        let clients = self.clients.lock().await;
        if let Some(sender) = clients.get(&client_id) {
            let _ = sender.send(log);
        }
    }

    pub async fn broadcast_log(&self, log: LogMessage) {
        let clients = self.clients.lock().await;
        let log_all = self.log_all_messages.lock().await;
        let mut dead_clients = Vec::new();

        for (client_id, sender) in clients.iter() {
            // Check if message is targeted
            if let Some(target_id) = &log.target_client_id {
                // Only send if this is the target client
                if target_id == client_id {
                    if let Err(_) = sender.send(log.clone()) {
                        dead_clients.push(client_id.clone());
                    }
                }
            } else {
                // Broadcast to all if no target specified
                if *log_all || !is_internal_message(&log) {
                    if let Err(_) = sender.send(log.clone()) {
                        dead_clients.push(client_id.clone());
                    }
                }
            }
        }

        // Clean up disconnected clients
        drop(clients);
        if !dead_clients.is_empty() {
            let mut clients = self.clients.lock().await;
            for client_id in dead_clients {
                clients.remove(&client_id);
                warn!("Removed disconnected client: {}", client_id);
            }
        }
    }

    pub async fn check_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Try to acquire the lock on clients - if we can't, server might be down
        if let Ok(clients) = self.clients.try_lock() {
            // Check if we have any active clients
            if !clients.is_empty() {
                return Ok(());
            }
        }

        // If we got here, either lock failed or no clients - consider connection dead
        Err("Connection lost to logging server".into())
    }
}

#[tonic::async_trait]
impl LogService for LoggingService {
    type SubscribeToLogsStream = Pin<Box<dyn Stream<Item = Result<LogMessage, Status>> + Send>>;

    async fn subscribe_to_logs(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeToLogsStream>, Status> {
        // Get metadata before consuming the request
        let metadata = request.metadata();
        info!("📝 Request headers: {:?}", metadata);

        // let receiver = self.sender.subscribe();
        // let stream = BroadcastStream::new(receiver);

        // Now consume the request to get client_id
        let client_id = request.into_inner().client_id;
        info!("🔌 New client connected: {}", client_id);

        // Create a channel for this specific client
        let (tx, rx) = mpsc::unbounded_channel();

        // Store the sender in our clients map
        {
            let mut clients = self.clients.lock().await;
            clients.insert(client_id.clone(), tx);
        }

        // Convert receiver into a stream
        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

        // let client_id_for_map = client_id.clone();
        // let client_id_for_end = client_id.clone();

        // Add test message right after connection
        let test_message = LogMessage {
            target_client_id: None,
            server_id: None, // Some("grpc-logger".to_string()),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            level: Some("INFO".to_string()),
            message: format!("Test message for client {}", client_id),
            target: None,    // Some("grpc_logger".to_string()),
            thread_id: None, // Some("main".to_string()),
            file: None,      //Some("server.rs".to_string()),
            line: None,      // Some("1".to_string()),
        };
        let client_id_for_end = client_id.clone();
        let client_id_for_log = client_id.clone();

        self.broadcast_log(test_message).await;

        let mapped_stream = Box::pin(
            stream
                .map(move |result| {
                    // Log when sending a message
                    if let Some(target) = &result.target {
                        if !target.starts_with("h2::")
                            && !target.starts_with("tonic::")
                            && !target.starts_with("tonic_web::")
                        //&& target == "grpc_logger"
                        {
                            info!(
                                "📤 Sending log to client {}: {:?}",
                                client_id_for_log, result
                            );
                        }
                    }
                    Ok(result)

                    // match &result {
                    //     Ok(log) => {
                    //         // Use as_ref() to get a reference to the String inside Option
                    //         if let Some(target) = log.target.as_ref() {
                    //             if !target.starts_with("h2::")
                    //                 && !target.starts_with("tonic::")
                    //                 && !target.starts_with("tonic_web::")
                    //                 && target == "grpc_logger"
                    //             {
                    //                 info!(
                    //                     "📤 Sending log to client {}: {:?}",
                    //                     client_id_for_map, log
                    //                 );
                    //             }
                    //         }
                    //     }
                    //     Err(e) => {
                    //         tracing::error!("❌ Error for client {}: {:?}", client_id_for_map, e)
                    //     }
                    // }
                    // map_broadcast_result(result)
                })
                .chain(futures::stream::once(async move {
                    info!("🏁 Stream ending for client {}", client_id_for_end);
                    Err(Status::ok("Stream complete"))
                })),
        );

        info!("✅ Stream setup complete for client: {}", client_id);
        Ok(Response::new(mapped_stream))
    }
}

// fn map_broadcast_result(
//     result: Result<LogMessage, BroadcastStreamRecvError>,
// ) -> Result<LogMessage, Status> {
//     match result {
//         Ok(msg) => Ok(msg),
//         Err(BroadcastStreamRecvError::Lagged(n)) => {
//             tracing::error!("Client lagging behind by {} messages", n);
//             Err(Status::resource_exhausted(format!(
//                 "Client lagging behind by {} messages",
//                 n
//             )))
//         }
//     }
// }

fn is_internal_message(log: &LogMessage) -> bool {
    const INTERNAL_PREFIXES: &[&str] = &[
        "h2::",
        "tonic::",
        "hyper::",
        "tower::",
        "runtime::",
        "http::",
    ];

    if let Some(target) = &log.target {
        INTERNAL_PREFIXES
            .iter()
            .any(|prefix| target.starts_with(prefix))
    } else {
        false
    }
}
