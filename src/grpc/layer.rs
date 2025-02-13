use crate::config::LogFieldsConfig;
use crate::server_build::logging::LogMessage;
use crate::server_build::LoggingService;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;

pub struct GrpcLayer {
    pub service: LoggingService,
    pub config: LogFieldsConfig,
    pub server_id: Option<String>,
}

impl<S> Layer<S> for GrpcLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Define prefixes to filter in a single array
        const INTERNAL_PREFIXES: &[&str] = &[
            // "h2::",
            // "tonic::",
            // "hyper::",
            // "tower::",
            // "runtime::",
            // "http::",
        ];

        // Check if target starts with any internal prefix
        let target = event.metadata().target();
        if INTERNAL_PREFIXES
            .iter()
            .any(|prefix| target.starts_with(prefix))
        {
            return;
        }

        struct LogVisitor {
            message: String,
            server_id: String,
            client_id: Option<String>,
        }

        impl Visit for LogVisitor {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                match field.name() {
                    "message" => self.message = format!("{:?}", value),
                    "server_id" => self.server_id = format!("{:?}", value),
                    "client_id" => self.client_id = Some(format!("{:?}", value)),
                    _ => {}
                }
            }
        }

        let mut visitor = LogVisitor {
            message: String::new(),
            server_id: String::new(),
            client_id: None,
        };
        event.record(&mut visitor);

        // Define message patterns to filter
        const FILTERED_PATTERNS: &[&str] = &[
            // "Queue::",
            // "transition_after",
            // "notifying task",
            // "assigned capacity",
            // "assigning",
            // "schedule_send",
            // "handshake complete",
            // "spawning background",
            // "checkout dropped",
            // "connection established",
            // "connection closed",
            // "dispatcher task",
            // "poll_ready",
            // "connection error",
            // "binding to",
            // "accept",
            // "http1 connection",
        ];

        // Skip if message is empty or contains any filtered pattern
        if visitor.message.trim().is_empty()
            || FILTERED_PATTERNS
                .iter()
                .any(|pattern| visitor.message.contains(pattern))
        {
            return;
        }

        let log = LogMessage {
            timestamp: if self.config.include_timestamp {
                Some(chrono::Local::now().to_rfc3339())
            } else {
                None
            },
            target_client_id: None,
            level: Some(event.metadata().level().to_string()),
            message: visitor.message,
            server_id: self.server_id.clone(),
            target: if self.config.include_target {
                Some(target.to_string())
            } else {
                None
            },
            thread_id: if self.config.include_thread_id {
                Some(format!("{:?}", std::thread::current().id()))
            } else {
                None
            },
            file: if self.config.include_file {
                Some(event.metadata().file().unwrap_or("unknown").to_string())
            } else {
                None
            },
            line: if self.config.include_line {
                Some(event.metadata().line().unwrap_or(0).to_string())
            } else {
                None
            },
        };

        // Create a cloned service for the spawn
        let service = self.service.clone();

        // If there's a client_id in the span, create a filtered broadcast
        if let Some(client_id) = visitor.client_id {
            let log_clone = log.clone();
            let client_id_clone = client_id.clone();
            tokio::spawn(async move {
                service
                    .broadcast_log_filtered(log_clone, client_id_clone)
                    .await;
            });
        } else {
            let log_clone = log.clone();
            tokio::spawn(async move {
                service.broadcast_log(log_clone).await;
            });
        }
    }
}
