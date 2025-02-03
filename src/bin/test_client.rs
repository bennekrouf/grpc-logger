//use logger_to_client::server_build::logging::log_service_client::LogServiceClient;
//use logger_to_client::server_build::logging::SubscribeRequest;
use tonic::Request;

pub mod logging {
    tonic::include_proto!("logging");
}

use logging::log_service_client::LogServiceClient;
use logging::SubscribeRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = LogServiceClient::connect("http://0.0.0.0:50052").await?;

    let request = SubscribeRequest {
        client_id: "test-client-1".to_string(),
    };

    let mut stream = client
        .subscribe_to_logs(Request::new(request))
        .await?
        .into_inner();

    println!("Connected to log server. Waiting for logs...");

    while let Some(log) = stream.message().await? {
        // Only show logs from your application
        if !log.target.starts_with("h2::") && !log.target.starts_with("tonic::") {
            println!("Received log: {:?}", log);
        }
    }

    Ok(())
}
