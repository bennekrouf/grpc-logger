use crate::grpc::GrpcConfig;
use serde::Deserialize;
use std::fs;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;

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
    pub server_id: Option<String>, // Add server_id field
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
    #[serde(default = "default_log_all_messages")]
    pub log_all_messages: bool,
}

fn default_log_all_messages() -> bool {
    false // By default, don't log all messages
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            output: LogOutput::Console, // or whatever default you prefer
            level: "info".to_string(),
            server_id: None,
            file_path: None,
            file_name: None,
            grpc: None,
            server_retry: ServerRetryConfig::default(),
            client_retry: ClientRetryConfig::default(),
            log_fields: LogFieldsConfig::default(),
            debug_mode: DebugConfig::default(),
            log_all_messages: false,
        }
    }
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
pub struct CustomTimer;
impl FormatTime for CustomTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let time = chrono::Local::now();
        write!(w, "{}", time.format("[%Y-%m-%d %H:%M:%S]"))
    }
}

// Custom format struct
#[derive(Clone)]
pub struct CustomFormatter {
    pub server_id: Option<String>,
    pub config: LogFieldsConfig,
}

impl<S, N> fmt::FormatEvent<S, N> for CustomFormatter
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    N: for<'writer> fmt::FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        self.write_timestamp(&mut writer)?;
        self.write_level(&mut writer, event)?;
        self.write_metadata(&mut writer, event)?;
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

impl CustomFormatter {
    fn write_timestamp(&self, writer: &mut Writer<'_>) -> std::fmt::Result {
        let time = chrono::Local::now();
        write!(writer, "[{}] ", time.format("%Y-%m-%d %H:%M:%S"))
    }

    fn write_level(&self, writer: &mut Writer<'_>, event: &tracing::Event<'_>) -> std::fmt::Result {
        write!(writer, "{:>5} ", event.metadata().level())
    }

    fn write_metadata(
        &self,
        writer: &mut Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        if let Some(server_id) = &self.server_id {
            write!(writer, "[{}] ", server_id)?;
        }

        if self.config.include_target
            && event.metadata().target() != "tokio_util::codec::framed_impl"
        {
            write!(writer, "{} - ", event.metadata().target())?;
        }

        // Additional metadata fields...
        self.write_location_info(writer, event)
    }

    fn write_location_info(
        &self,
        writer: &mut Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        if self.config.include_file {
            if let Some(file) = event.metadata().file() {
                write!(writer, "{}:", file)?;
                if self.config.include_line {
                    if let Some(line) = event.metadata().line() {
                        write!(writer, "{} ", line)?;
                    }
                }
            }
        }
        Ok(())
    }
}

// Configuration and setup functions
pub fn load_config(path: &str) -> Result<LogConfig, Box<dyn std::error::Error + Send + Sync>> {
    let config_str = fs::read_to_string(path)?;
    let config: LogConfig = serde_yaml::from_str(&config_str)?;
    Ok(config)
}


