use crate::grpc::GrpcConfig;
use crate::grpc::GrpcLayer;
use crate::server_build::LoggingService;
use serde::Deserialize;
use std::fs;
use std::io;
use tracing::Level;
use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::Registry;

// Configuration structs
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    Console,
    File,
    Grpc,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct LogFieldsConfig {
    pub include_thread_id: bool,
    pub include_target: bool,
    pub include_file: bool,
    pub include_line: bool,
    pub include_timestamp: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct DebugConfig {
    pub enabled: bool,
    pub test_interval_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct LogConfig {
    pub output: LogOutput,
    pub level: String,
    pub whoami: Option<String>, // Add whoami field
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub grpc: Option<GrpcConfig>,
    #[serde(default)]
    pub server_retry: ServerRetryConfig,
    #[serde(default)]
    pub client_retry: ClientRetryConfig,
    #[serde(default)]
    pub log_fields: LogFieldsConfig,
    #[serde(default)]
    pub debug_mode: DebugConfig,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ServerRetryConfig {
    pub max_retries: u32,     // Fewer retries, maybe 5-10
    pub base_delay_secs: u64, // Shorter delays
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ClientRetryConfig {
    pub max_retries: u32,          // More retries, like 5000
    pub base_delay_secs: u64,      // Can be longer
    pub reconnect_delay_secs: u64, // For maintaining connection
}

// Timer formatting
struct CustomTimer;
impl FormatTime for CustomTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let time = chrono::Local::now();
        write!(w, "{}", time.format("[%Y-%m-%d %H:%M:%S]"))
    }
}

// Custom format struct
#[derive(Clone)]
struct CustomFormatter {
    whoami: Option<String>,
    config: LogFieldsConfig,
}

impl<S, N> fmt::FormatEvent<S, N> for CustomFormatter
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    N: for<'writer> fmt::FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        // Write timestamp
        let time = chrono::Local::now();
        write!(writer, "{}", time.format("[%Y-%m-%d %H:%M:%S]"))?;
        writer.write_char(' ')?;

        // Write level
        let level = *event.metadata().level();
        write!(writer, "{:>5} ", level)?;

        // Write whoami prefix if present
        if let Some(whoami) = &self.whoami {
            write!(writer, "[{}] ", whoami)?;
        }

        // Write target if configured
        if self.config.include_target
            && event.metadata().target() != "tokio_util::codec::framed_impl"
        {
            write!(writer, "{} - ", event.metadata().target())?;
        }

        // Write thread ID if configured
        if self.config.include_thread_id {
            write!(writer, "thread={:?} ", std::thread::current().id())?;
        }

        // Write file and line if configured
        if self.config.include_file {
            if let Some(file) = event.metadata().file() {
                write!(writer, "{}:", file)?;
                if self.config.include_line {
                    if let Some(line) = event.metadata().line() {
                        write!(writer, "{} ", line)?;
                    }
                } else {
                    write!(writer, " ")?;
                }
            }
        } else if self.config.include_line {
            if let Some(line) = event.metadata().line() {
                write!(writer, "line={} ", line)?;
            }
        }

        // Write the actual message
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
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

    let env_filter = EnvFilter::new("")
        .add_directive("logger_to_client=info".parse()?)
        .add_directive("warn".parse()?);

    let subscriber = Registry::default();
    let format = CustomFormatter {
        whoami: config.whoami.clone(),
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
                },
            )))
            .expect("Failed to set subscriber");
            Ok(None)
        }
    }
}
