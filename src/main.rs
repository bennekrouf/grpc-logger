use std::io;
use tracing::{info, Level};
use tracing_subscriber::{fmt::time::FormatTime, Layer, Registry};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt};
use tracing_subscriber::fmt;
use tracing_appender::non_blocking::NonBlocking;

#[derive(Debug, Clone, Copy)]
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

fn setup_logging(output: LogOutput) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::from_default_env()
        .add_directive(Level::INFO.into());

    match output {
        LogOutput::File => {
            let file_appender = RollingFileAppender::new(
                Rotation::NEVER,
                "logs",
                "app.log",
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

            Some(guard)
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

            None
        }
    }
}

fn main() {
    let output = LogOutput::File; // or LogOutput::Console
    let _guard = setup_logging(output);
    info!(target: "my_logger", "MyApp: Application started");
}
