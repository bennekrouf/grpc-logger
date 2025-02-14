use std::io::Write;
use tokio::sync::mpsc;
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone)]
pub struct GrpcWriter {
    sender: mpsc::UnboundedSender<String>,
}

impl<'a> MakeWriter<'a> for GrpcWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Write for GrpcWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(log_str) = String::from_utf8(buf.to_vec()) {
            self.sender
                .send(log_str)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
