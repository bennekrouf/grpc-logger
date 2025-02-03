mod server_build;
use server_build::logging::log_service_server::LogServiceServer;

use std::io::{self, Write};
use std::fs;
use tracing::Level;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::EnvFilter;
use tracing_appender::non_blocking::NonBlocking;
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing_subscriber::fmt::MakeWriter;
use tokio::sync::broadcast;
use tonic::transport::Server;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;
use tracing_subscriber::Layer;
// use logging::LogMessage;
use server_build::LoggingService;

// Configuration structs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LogOutput {
    Console,
    File,
    Grpc,
}

#[derive(Debug, Deserialize)]
struct GrpcConfig {
    address: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct LogConfig {
    output: LogOutput,
    level: String,
    file_path: Option<String>,
    file_name: Option<String>,
    grpc: Option<GrpcConfig>,
}

// GRPC Writers
#[derive(Clone)]
struct GrpcWriter {
    sender: mpsc::UnboundedSender<String>,
}

impl<'a> MakeWriter<'a> for GrpcWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Write for GrpcWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(log_str) = String::from_utf8(buf.to_vec()) {
            self.sender.send(log_str).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct BatchingGrpcWriter {
    sender: mpsc::UnboundedSender<String>,
    buffer: Vec<String>,
    buffer_size: usize,
}

impl BatchingGrpcWriter {
    fn new(sender: mpsc::UnboundedSender<String>, buffer_size: usize) -> Self {
        Self {
            sender,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
        }
    }
}

impl Write for BatchingGrpcWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(log_str) = String::from_utf8(buf.to_vec()) {
            self.buffer.push(log_str);

            if self.buffer.len() >= self.buffer_size {
                let logs = self.buffer.join("\n");
                self.sender.send(logs).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                })?;
                self.buffer.clear();
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            let logs = self.buffer.join("\n");
            self.sender.send(logs).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;
            self.buffer.clear();
        }
        Ok(())
    }
}

// Create a custom layer that forwards logs to gRPC clients
struct GrpcLayer {
    service: LoggingService,
}

impl<S> Layer<S> for GrpcLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        use tracing::field::{Field, Visit};
        struct LogVisitor {
            message: String,
        }

        impl Visit for LogVisitor {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{:?}", value);
                }
            }
        }

        let mut visitor = LogVisitor {
            message: String::new(),
        };
        
        event.record(&mut visitor);

        // Get a string representation of the thread id without using as_u64()
        let thread_id = format!("{:?}", std::thread::current().id());

        let log = LogMessage {
            timestamp: chrono::Local::now().to_rfc3339(),
            level: event.metadata().level().to_string(),
            message: visitor.message,
            target: event.metadata().target().to_string(),
            thread_id,  // Using the debug format of ThreadId instead
            file: event.metadata().file().unwrap_or("unknown").to_string(),
            line: event.metadata().line().unwrap_or(0).to_string(),
        };

        self.service.broadcast_log(log);
    }
}

// Timer formatting
struct CustomTimer;
impl FormatTime for CustomTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let time = chrono::Local::now();
        write!(w, "{}", time.format("[%Y-%m-%d %H:%M:%S]"))
    }
}

// Configuration and setup functions
fn load_config(path: &str) -> Result<LogConfig, Box<dyn std::error::Error>> {
    let config_str = fs::read_to_string(path)?;
    let config: LogConfig = serde_yaml::from_str(&config_str)?;
    Ok(config)
}

fn setup_logging(config: &LogConfig, grpc_service: Option<LoggingService>) 
    -> Result<Option<tracing_appender::non_blocking::WorkerGuard>, Box<dyn std::error::Error>> 
{
    let level = match config.level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let _filter = EnvFilter::from_default_env()
        .add_directive(level.into());

    let subscriber = Registry::default();

    match config.output {
        LogOutput::File => {
            let file_path = config.file_path.as_deref().unwrap_or("logs");
            let file_name = config.file_name.as_deref().unwrap_or("app.log");

            let file_appender = RollingFileAppender::new(
                Rotation::NEVER,
                file_path,
                file_name,
            );
            let (non_blocking, guard) = NonBlocking::new(file_appender);

            let layer = layer()
                .with_writer(non_blocking)
                .with_timer(CustomTimer)
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(false)
                .with_level(true)
                .with_thread_names(true)
                .with_filter(EnvFilter::from_default_env().add_directive(level.into()));

            tracing::subscriber::set_global_default(
                subscriber
                    .with(layer)
                    .with(grpc_service.map(|service| GrpcLayer { service }))
            ).expect("Failed to set subscriber");

            Ok(Some(guard))
        },
        LogOutput::Console | LogOutput::Grpc => {
            let layer = layer()
                .with_writer(io::stdout)
                .with_timer(CustomTimer)
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(false)
                .with_level(true)
                .with_thread_names(true)
                .with_filter(EnvFilter::from_default_env().add_directive(level.into()));

            tracing::subscriber::set_global_default(
                subscriber
                    .with(layer)
                    .with(grpc_service.map(|service| GrpcLayer { service }))
            ).expect("Failed to set subscriber");

            Ok(None)
        }
    }
}

use crate::server_build::logging::LogMessage;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config("config.yaml")?;
    
    // Create a single instance of the gRPC service
    let service = LoggingService::new();
    let service_clone = service.clone();

    // Set up logging with the cloned service
    let _guard = setup_logging(&config, Some(service_clone))?;

    // Start the gRPC server with the original service
    let addr = "[::1]:50051".parse()?;
    println!("Log Server listening on {}", addr);

    Server::builder()
        .add_service(LogServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
