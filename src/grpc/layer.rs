use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;

use crate::server_build::logging::LogMessage;
use crate::server_build::LoggingService;

pub struct GrpcLayer {
    pub service: LoggingService,
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
        let thread_id = format!("{:?}", std::thread::current().id());

        let log = LogMessage {
            timestamp: chrono::Local::now().to_rfc3339(),
            level: event.metadata().level().to_string(),
            message: visitor.message,
            target: event.metadata().target().to_string(),
            thread_id,
            file: event.metadata().file().unwrap_or("unknown").to_string(),
            line: event.metadata().line().unwrap_or(0).to_string(),
        };

        self.service.broadcast_log(log);
    }
}
