use tokio::sync::broadcast;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::BroadcastStream;
use futures::StreamExt;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use std::pin::Pin;
use futures::Stream;
use tracing::warn;

pub mod logging {
    tonic::include_proto!("logging");
}

use logging::log_service_server::LogService;
use logging::{LogMessage, SubscribeRequest};

#[derive(Debug, Clone)]
pub struct LoggingService {
    pub sender: broadcast::Sender<LogMessage>,
}

impl LoggingService {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }

    pub fn broadcast_log(&self, log: LogMessage) {
        // Only try to send if there are active receivers
        if self.sender.receiver_count() > 0 {
            if let Err(e) = self.sender.send(log) {
                warn!("Failed to broadcast log: {}", e);
            }
        }
    }
}

#[tonic::async_trait]
impl LogService for LoggingService {

    type SubscribeToLogsStream = Pin<Box<dyn Stream<Item = Result<LogMessage, Status>> + Send>>;

    async fn subscribe_to_logs(
        &self,
        request: Request<SubscribeRequest>
    ) -> 
        Result<Response<Self::SubscribeToLogsStream>, Status> {
        println!("New client connected: {}", request.into_inner().client_id);

        let receiver = self.sender.subscribe();
        let stream = BroadcastStream::new(receiver);

        let mapped_stream = Box::pin(stream.map(|result| map_broadcast_result(result)));
        Ok(Response::new(mapped_stream))
    }
}

fn map_broadcast_result(
    result: Result<LogMessage, BroadcastStreamRecvError>,
) -> Result<LogMessage, Status> {
    result.map_err(|e| Status::internal(format!("Failed to receive log message: {}", e)))
}
