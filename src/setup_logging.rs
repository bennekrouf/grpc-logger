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

pub async fn setup_logging(config: &LogConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let level = match config.level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    // Create a new LoggingService for forwarding logs
    let logging_service = LoggingService::new();

    let subscriber = Registry::default();
    let fmt_layer = layer()
        .with_writer(io::stdout)
        .with_timer(CustomTimer)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_ansi(false)
        .with_level(true)
        .with_thread_names(false)
        .with_filter(EnvFilter::new("")
            .add_directive("logger_to_client=info".parse()?)
            .add_directive("warn".parse()?)
            .add_directive(level.into()));

    // Connect to gRPC logger if configured
    if let Some(grpc_config) = &config.grpc {
        let addr = format!("http://{}:{}", grpc_config.address, grpc_config.port);
        println!("Attempting to connect to grpc-logger at {}", addr);
        
        let server_id = config.server_id.clone()
            .unwrap_or_else(|| format!("server-{}", Uuid::new_v4()));
        println!("Creating subscription request for server: {}", server_id);
        
        let request = Request::new(SubscribeRequest {
            client_id: server_id.clone(),
            client_type: ClientType::Server as i32,
            server_name: server_id.clone(),
        });

        // Connect to gRPC server
        let mut client = LogServiceClient::connect(addr).await?;
        println!("Connected to gRPC server");
        
        // Start the subscription in a separate task
        let response = client.subscribe_to_logs(request).await?;
        let mut stream = response.into_inner();
        
        // Spawn a task to handle the stream
        tokio::spawn(async move {
            while let Ok(Some(log)) = stream.message().await {
                println!("Received log from server: {:?}", log);
            }
            println!("Stream ended");
        });

        println!("Successfully subscribed to grpc-logger");

        // Create the GRPC layer
        let grpc_layer = GrpcLayer {
            service: logging_service,
            config: config.log_fields.clone(),
            server_id: Some(server_id),
        };

        // Set up the subscriber with both layers
        tracing::subscriber::set_global_default(subscriber.with(fmt_layer).with(grpc_layer))
            .expect("Failed to set subscriber");
    } else {
        // Just use the standard subscriber
        tracing::subscriber::set_global_default(subscriber.with(fmt_layer))
            .expect("Failed to set subscriber");
    }

    println!("Logging setup complete");
    Ok(())
}

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

