use crate::grpc::GrpcLayer;
use serde::Deserialize;
use std::fs;
use std::io;
use tracing::Level;
use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;
use crate::server_build::LoggingService;
use crate::grpc::GrpcConfig;

// Configuration structs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    Console,
    File,
    Grpc,
}

#[derive(Debug, Deserialize)]
pub struct LogConfig {
    pub output: LogOutput,
    pub level: String,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub grpc: Option<GrpcConfig>,
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
pub fn load_config(path: &str) -> Result<LogConfig, Box<dyn std::error::Error>> {
    let config_str = fs::read_to_string(path)?;
    let config: LogConfig = serde_yaml::from_str(&config_str)?;
    Ok(config)
}

pub fn setup_logging(
    config: &LogConfig,
    grpc_service: Option<LoggingService>,
) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>, Box<dyn std::error::Error>> {
    let level = match config.level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    //let _filter = EnvFilter::from_default_env().add_directive(level.into());
    EnvFilter::new("")
        .add_directive("logger_to_client=info".parse()?) // Your app logs
        .add_directive("warn".parse()?);

    let subscriber = Registry::default();

    match config.output {
        LogOutput::File => {
            let file_path = config.file_path.as_deref().unwrap_or("logs");
            let file_name = config.file_name.as_deref().unwrap_or("app.log");

            let file_appender = RollingFileAppender::new(Rotation::NEVER, file_path, file_name);
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
                    .with(grpc_service.map(|service| GrpcLayer { service })),
            )
            .expect("Failed to set subscriber");

            Ok(Some(guard))
        }
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
                    .with(grpc_service.map(|service| GrpcLayer { service })),
            )
            .expect("Failed to set subscriber");

            Ok(None)
        }
    }
}
