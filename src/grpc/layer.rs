use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;
use crate::server_build::logging::LogMessage;
use crate::server_build::LoggingService;
use crate::config::LogFieldsConfig;

pub struct GrpcLayer {
    pub service: LoggingService,
    pub config: LogFieldsConfig,
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
            "h2::",
            "tonic::",
            "hyper::",
            "tower::",
        ];

        // Check if target starts with any internal prefix
        let target = event.metadata().target();
        if INTERNAL_PREFIXES.iter().any(|prefix| target.starts_with(prefix)) {
            return;
        }

        struct LogVisitor {
            message: String,
        }

        impl Visit for LogVisitor {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{:?}", value);
                }
            }
        }

        let mut visitor = LogVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        // Define message patterns to filter
        const FILTERED_PATTERNS: &[&str] = &[
            "Queue::",
            "transition_after",
            "notifying task",
            "assigned capacity",
            "assigning",
            "schedule_send",
        ];

        // Skip if message is empty or contains any filtered pattern
        if visitor.message.trim().is_empty() || 
           FILTERED_PATTERNS.iter().any(|pattern| visitor.message.contains(pattern)) {
            return;
        }

        let log = LogMessage {
            timestamp: if self.config.include_timestamp {
                Some(chrono::Local::now().to_rfc3339())
            } else {
                None
            },
            level: Some(event.metadata().level().to_string()),
            message: visitor.message,
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

        self.service.broadcast_log(log);
    }
}
