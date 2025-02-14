use crate::grpc::GrpcLayer;
use crate::server_build::LoggingService;
use tracing::Level;
use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use std::io;

use crate::config::CustomTimer;
use tracing_subscriber::Layer;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use crate::config::CustomFormatter;
use crate::config::LogConfig;
use tracing_subscriber::Registry;
use crate::config::LogOutput;
use tracing_subscriber::fmt::layer;
use crate::server_build::logging::log_service_client::LogServiceClient;
use tonic::Request;
use uuid::Uuid;
use crate::server_build::logging::ClientType;
use crate::server_build::logging::SubscribeRequest;


pub async fn setup_logging(
    config: &LogConfig,
    // _grpc_service: Option<LoggingService>,
) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>, Box<dyn std::error::Error + Send + Sync>> {
    // If we have GRPC configuration, connect to the GRPC logger first
    if let Some(grpc_config) = &config.grpc {
        let addr = format!("http://{}:{}", grpc_config.address, grpc_config.port);
        println!("Attempting to connect to grpc-logger at {}", addr);
        
        // Create the subscribe request with server type
        let server_id = config.server_id.clone().unwrap_or_else(|| format!("server-{}", Uuid::new_v4()));
        println!("Creating subscription request for server: {}", server_id);
        
        let request = Request::new(SubscribeRequest {
            client_id: server_id.clone(),
            client_type: ClientType::Server as i32,
            server_name: server_id,
        });

        // Connect to gRPC server
        println!("Connecting to gRPC server...");
        let mut client = LogServiceClient::connect(addr).await?;
        println!("Connected successfully!");
        
        // Subscribe to logs
        println!("Subscribing to logs...");
        let _response = client.subscribe_to_logs(request).await?;
        println!("Subscribed successfully!");
    }

    // Create a new logging service for internal use
    let log_service = LoggingService::new();

    // Then proceed with regular logging setup
    println!("Setting up internal logging...");
    setup_logging_internal(config, Some(log_service)).await}

pub async fn setup_logging_internal(
    config: &LogConfig,
    grpc_service: Option<LoggingService>,
) -> Result<
    Option<tracing_appender::non_blocking::WorkerGuard>,
    Box<dyn std::error::Error + Sync + Send>,
> {
    let level = match config.level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let env_filter = EnvFilter::new("")
        .add_directive("logger_to_client=info".parse()?)
        .add_directive("warn".parse()?);

    let subscriber = Registry::default();
    let format = CustomFormatter {
        server_id: config.server_id.clone(),
        config: config.log_fields.clone(),
    };

    match config.output {
        LogOutput::File => {
            let file_path = config.file_path.as_deref().unwrap_or("logs");
            let file_name = config.file_name.as_deref().unwrap_or("app.log");

            let file_appender = RollingFileAppender::new(Rotation::NEVER, file_path, file_name);
            let (non_blocking, guard) = NonBlocking::new(file_appender);

            let layer = layer()
                .event_format(format)
                .with_writer(non_blocking)
                .with_filter(env_filter.add_directive(level.into()));

            tracing::subscriber::set_global_default(subscriber.with(layer).with(grpc_service.map(
                |service| GrpcLayer {
                    service,
                    config: config.log_fields.clone(),
                    server_id: config.server_id.clone(),
                },
            )))
            .expect("Failed to set subscriber");
            Ok(Some(guard))
        }
        LogOutput::Console | LogOutput::Grpc => {
            let layer = layer()
                .with_writer(io::stdout)
                .with_timer(CustomTimer)
                .with_target(false)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .with_ansi(false)
                .with_level(true)
                .with_thread_names(false)
                .with_filter(EnvFilter::from_default_env().add_directive(level.into()));

            tracing::subscriber::set_global_default(subscriber.with(layer).with(grpc_service.map(
                |service| GrpcLayer {
                    service,
                    config: config.log_fields.clone(),
                    server_id: config.server_id.clone(),
                },
            )))
            .expect("Failed to set subscriber");
            Ok(None)
        }
    }
}

