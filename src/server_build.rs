// src/server_build.rs
use tokio::sync::broadcast;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::BroadcastStream;
// use tokio_stream::StreamExt; 
use futures::StreamExt;
pub mod logging {
    tonic::include_proto!("logging");
}

use logging::log_service_server::LogService;
use logging::{LogMessage, SubscribeRequest};
use futures::stream::Map;
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
        if let Err(e) = self.sender.send(log) {
            eprintln!("Failed to broadcast log: {}", e);
        }
    }
}


use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use std::pin::Pin;
use futures::Stream;

#[tonic::async_trait]
impl LogService for LoggingService {
    // type SubscribeToLogsStream = Map<
    //     BroadcastStream<LogMessage>,
    //     fn(Result<LogMessage, tokio_stream::wrappers::errors::BroadcastStreamRecvError>) -> Result<LogMessage, Status>,
    // >;

// type SubscribeToLogsStream = Map<
//     BroadcastStream<LogMessage>,
//     fn(Result<LogMessage, BroadcastStreamRecvError>) -> Result<LogMessage, Status>,
// >;

// type SubscribeToLogsStream = impl Stream<Item = Result<LogMessage, Status>>;

// type SubscribeToLogsStream = impl Stream<Item = Result<LogMessage, Status>>;
type SubscribeToLogsStream = Pin<Box<dyn Stream<Item = Result<LogMessage, Status>> + Send>>;
    async fn subscribe_to_logs(
        &self,
        request: Request<SubscribeRequest>
    ) -> 
        Result<Response<Self::SubscribeToLogsStream>, Status> {
        // Result<Response<Self::SubscribeToLogsStream>, Status> {
        println!("New client connected: {}", request.into_inner().client_id);
        
        let receiver = self.sender.subscribe();
        let stream = BroadcastStream::new(receiver);
        
// let mapped_stream = stream.map(|result| map_broadcast_result(result));

        // Map the error type from `BroadcastStreamRecvError` to `tonic::Status`
        // let mapped_stream = stream.map(|result| {
        //     result.map_err(|e| {
        //         Status::internal(format!("Failed to receive log message: {}", e))
        //     })
        // });

        // let mapped_stream = stream.map(map_broadcast_result);
    let mapped_stream = Box::pin(stream.map(|result| map_broadcast_result(result)));
        Ok(Response::new(mapped_stream))
    }
}

fn map_broadcast_result(
    result: Result<LogMessage, BroadcastStreamRecvError>,
) -> Result<LogMessage, Status> {
    result.map_err(|e| Status::internal(format!("Failed to receive log message: {}", e)))
}
