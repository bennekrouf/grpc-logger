use std::io;
use std::fs;
use tracing::{info, Level};
use tracing_subscriber::{fmt::time::FormatTime, fmt};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::EnvFilter;
use tracing_appender::non_blocking::NonBlocking;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LogConfig {
    output: LogOutput,
    level: String,
    file_path: Option<String>,
    file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LogOutput {
    Console,
    File,
}

struct CustomTimer;
impl FormatTime for CustomTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let time = chrono::Local::now();
        write!(w, "{}", time.format("[%Y-%m-%d %H:%M:%S]"))
    }
}

fn load_config(path: &str) -> Result<LogConfig, Box<dyn std::error::Error>> {
    let config_str = fs::read_to_string(path)?;
    let config: LogConfig = serde_yaml::from_str(&config_str)?;
    Ok(config)
}

fn setup_logging(config: &LogConfig) -> Result<Option<tracing_appender::non_blocking::WorkerGuard>, Box<dyn std::error::Error>> {
    let level = match config.level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let filter = EnvFilter::from_default_env()
        .add_directive(level.into());

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
            
            fmt::Subscriber::builder()
                .with_writer(non_blocking)
                .with_timer(CustomTimer)
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(false)
                .with_level(true)
                .with_thread_names(true)
                .with_env_filter(filter)
                .init();

            Ok(Some(guard))
        },
        LogOutput::Console => {
            fmt::Subscriber::builder()
                .with_writer(io::stdout)
                .with_timer(CustomTimer)
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(false)
                .with_level(true)
                .with_thread_names(true)
                .with_env_filter(filter)
                .init();

            Ok(None)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config("config.yaml")?;
    let _guard = setup_logging(&config)?;
    
    info!(target: "my_logger", "MyApp: Application started");
    Ok(())
}
