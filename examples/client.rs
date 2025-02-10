use grpc_logger::{config::load_config, LogConfig};
use std::time::Duration;
use tokio::time::sleep;
use tonic::Request;
use tracing::{debug, error, info};
use tracing_subscriber::{fmt, EnvFilter};
pub mod logging {
    tonic::include_proto!("logging");
}
use logging::log_service_client::LogServiceClient;
use logging::SubscribeRequest;

async fn connect_with_retry(
    config: &LogConfig,
) -> Result<LogServiceClient<tonic::transport::Channel>, tonic::transport::Error> {
    let mut retry_count = 0;
    let retry_config = &config.client_retry;
    let base_delay = Duration::from_secs(retry_config.base_delay_secs);
    let server_addr = format!(
        "http://{}:{}",
        config.grpc.as_ref().unwrap().address,
        config.grpc.as_ref().unwrap().port
    );
    loop {
        debug!("Attempting to connect to {}", server_addr);
        match LogServiceClient::connect(server_addr.clone()).await {
            Ok(client) => {
                info!("Successfully connected to log server at {}", server_addr);
                return Ok(client);
            }
            Err(e) => {
                retry_count += 1;
                if retry_count > retry_config.max_retries {
                    error!(
                        "Failed to connect after {} retries. Exiting.",
                        retry_config.max_retries
                    );
                    return Err(e);
                }
                let delay = base_delay.mul_f32(1.5f32.powi(retry_count as i32));
                error!(
                    "Failed to connect to server: {}. Retrying in {} seconds...",
                    e,
                    delay.as_secs()
                );
                sleep(delay).await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging for the client
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true)
        .with_span_events(fmt::format::FmtSpan::FULL) // Include span events
        // .fmt_fields(tracing_subscriber::fmt::format::FullFields::new())
        .pretty()
        .init();

    // Load configuration
    let config = load_config("examples/client.yaml")?;
    info!("Starting log client...");

    loop {
        let mut client = connect_with_retry(&config).await?;
        let request = SubscribeRequest {
            client_id: "rust-client-1".to_string(),
        };
        debug!("Subscribing to log stream...");

        match client
            .subscribe_to_logs(Request::new(request.clone()))
            .await
        {
            Ok(response) => {
                info!("Connected to log server. Waiting for logs...");
                let mut stream = response.into_inner();
                while let Some(log) = stream.message().await? {
                    // Check if target exists and filter internal logs
                    let should_display = log.target.as_ref().map_or(true, |target| {
                        !target.starts_with("h2::") && !target.starts_with("tonic::")
                    });

                    if should_display {
                        // Format log message with only available fields
                        let formatted_log = format!(
                            "Log {{ timestamp: {}, level: {}, message: '{}'{}{}{}{} }}",
                            log.timestamp.unwrap_or_default(),
                            log.level.unwrap_or_default(),
                            log.message,
                            log.target
                                .as_ref()
                                .map_or(String::new(), |t| format!(", target: '{}'", t)),
                            log.thread_id
                                .as_ref()
                                .map_or(String::new(), |t| format!(", thread: '{}'", t)),
                            log.file
                                .as_ref()
                                .map_or(String::new(), |f| format!(", file: '{}'", f)),
                            log.line
                                .as_ref()
                                .map_or(String::new(), |l| format!(", line: {}", l)),
                        );
                        info!("{}", formatted_log);
                    }
                }
                debug!("Stream ended. Attempting to reconnect...");
            }
            Err(e) => {
                error!(
                    "Lost connection to server: {}. Attempting to reconnect...",
                    e
                );
            }
        }
        sleep(Duration::from_secs(
            config.client_retry.reconnect_delay_secs,
        ))
        .await;
    }
}
