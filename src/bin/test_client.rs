use tonic::Request;
use std::time::Duration;
use tokio::time::sleep;

pub mod logging {
    tonic::include_proto!("logging");
}
use logging::log_service_client::LogServiceClient;
use logging::SubscribeRequest;

const SERVER_ADDR: &str = "http://0.0.0.0:50052";
const MAX_RETRIES: u32 = 5000;
const BASE_DELAY_SECS: u64 = 2;
const RECONNECT_DELAY_SECS: u64 = 2;

async fn connect_with_retry() -> Result<LogServiceClient<tonic::transport::Channel>, tonic::transport::Error> {
    let mut retry_count = 0;
    let base_delay = Duration::from_secs(BASE_DELAY_SECS);

    loop {
        println!("Attempting to connect to {}", SERVER_ADDR);
        match LogServiceClient::connect(SERVER_ADDR.to_string()).await {
            Ok(client) => {
                println!("Successfully connected to log server at {}", SERVER_ADDR);
                return Ok(client);
            }
            Err(e) => {
                retry_count += 1;
                if retry_count > MAX_RETRIES {
                    println!("Failed to connect after {} retries. Exiting.", MAX_RETRIES);
                    return Err(e);
                }
                let delay = base_delay.mul_f32(1.5f32.powi(retry_count as i32));
                println!("Failed to connect to server: {}. Retrying in {} seconds...", 
                    e, delay.as_secs());
                sleep(delay).await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting log client...");

    loop {
        let mut client = connect_with_retry().await?;
        let request = SubscribeRequest {
            client_id: "test-client-1".to_string(),
        };

        println!("Subscribing to log stream...");
        match client.subscribe_to_logs(Request::new(request.clone())).await {
            Ok(response) => {
                println!("Connected to log server. Waiting for logs...");
                let mut stream = response.into_inner();
                while let Some(log) = stream.message().await? {
                    if !log.target.starts_with("h2::") && !log.target.starts_with("tonic::") {
                        println!("Received log: {:?}", log);
                    }
                }
                println!("Stream ended. Attempting to reconnect...");
            }
            Err(e) => {
                println!("Lost connection to server: {}. Attempting to reconnect...", e);
            }
        }

        sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
    }
}

