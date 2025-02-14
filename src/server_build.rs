use crate::config::{LogConfig, LogOutput};
use crate::setup_logging::setup_logging;
use futures::Stream;
use futures::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use tracing::{info, trace, warn};

use crate::server_build::logging::ClientType;
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
        // let service_clone = self.clone();
        let _guard = setup_logging(config);

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

        // Skip internal messages unless explicitly configured to log all
        if !*log_all && is_internal_message(&log) {
            return;
        }

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
        println!("→ Entering subscribe_to_logs");
        info!("Starting new log subscription request");

        let metadata = request.metadata();
        println!("  Metadata received: {:?}", metadata);

        let request_inner = request.into_inner();
        let client_id = request_inner.client_id;
        println!("  Extracted client_id: {}", client_id);

        let client_type = ClientType::from_i32(request_inner.client_type)
            .unwrap_or(ClientType::Unknown);
        println!("  Client type resolved to: {:?}", client_type);

        // Log different messages based on client type
        match client_type {
            ClientType::Server => {
                let server_name = request_inner.server_name;
                println!("  Processing Server client type");
                info!(
                    "🔧 Server instance connected: {} (name: {})",
                    client_id, server_name
                );
            }
            ClientType::WebClient => {
                println!("  Processing WebClient client type");
                info!("🌐 Web client connected: {}", client_id);
            }
            ClientType::Unknown => {
                println!("  Processing Unknown client type");
                warn!("⚠️ Unknown client type connected: {}", client_id);
            }
        }

        // Create a channel for this specific client
        let (tx, rx) = mpsc::unbounded_channel();
        println!("  Channel created for client");

        // Store the sender in our clients map
        {
            println!("  Attempting to acquire clients lock");
            let mut clients = self.clients.lock().await;
            println!("  Lock acquired, inserting client");
            clients.insert(client_id.clone(), tx);
            info!("Added new client {} to clients map", client_id);
        }

        // Convert receiver into a stream
        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        println!("  Stream created from receiver");

        // Only send test message to web clients
        if client_type == ClientType::WebClient {
            println!("  Preparing test message for web client");
            let test_message = LogMessage {
                target_client_id: None,
                server_id: None,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                level: Some("INFO".to_string()),
                message: format!("Test message for web client {}", client_id),
                target: None,
                thread_id: None,
                file: None,
                line: None,
            };
            println!("  Broadcasting test message");
            self.broadcast_log(test_message).await;
            info!("Sent test message to web client {}", client_id);
        }

        let client_id_for_end = client_id.clone();
        let client_id_for_log = client_id.clone();
        println!("  Setting up mapped stream");

        let mapped_stream = Box::pin(
            stream
                .map(move |result| {
                    println!("  Processing stream message for client {}", client_id_for_log);
                    // Log when sending a message
                    if let Some(target) = &result.target {
                        if !target.starts_with("h2::")
                            && !target.starts_with("tonic::")
                            && !target.starts_with("tonic_web::")
                        {
                            info!(
                                "📤 Sending log to client {}: {:?}",
                                client_id_for_log, result
                            );
                        }
                    }
                    Ok(result)
                })
                .chain(futures::stream::once(async move {
                    println!("  Stream ending for client {}", client_id_for_end);
                    info!("🏁 Stream ending for client {}", client_id_for_end);
                    Err(Status::ok("Stream complete"))
                })),
        );

        println!("← Exiting subscribe_to_logs");
        info!("✅ Stream setup complete for client: {}", client_id);
        Ok(Response::new(mapped_stream))
    }
}

fn is_internal_message(log: &LogMessage) -> bool {
    const INTERNAL_PREFIXES: &[&str] = &[
        "h2::",
        "tonic::",
        "hyper::",
        "tower::",
        "runtime::",
        "http::",
        "Connection{peer=",  // Add this to catch the h2 connection messages
    ];

    const INTERNAL_PATTERNS: &[&str] = &[
        "send frame=",
        "transition_after",
        "writing frame=",
        "encoding RESET",
        "flushing buffer",
        "connection established",
        "connection closed",
        "send",
        "poll",
    ];

    if let Some(target) = &log.target {
        if INTERNAL_PREFIXES.iter().any(|prefix| target.starts_with(prefix)) {
            return true;
        }
    }

    INTERNAL_PATTERNS.iter().any(|pattern| log.message.contains(pattern))
}
