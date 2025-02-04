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

pub struct BatchingGrpcWriter {
    sender: mpsc::UnboundedSender<String>,
    buffer: Vec<String>,
    buffer_size: usize,
}

impl BatchingGrpcWriter {
    pub fn new(sender: mpsc::UnboundedSender<String>, buffer_size: usize) -> Self {
        Self {
            sender,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
        }
    }
}

impl Write for BatchingGrpcWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(log_str) = String::from_utf8(buf.to_vec()) {
            self.buffer.push(log_str);

            if self.buffer.len() >= self.buffer_size {
                let logs = self.buffer.join("\n");
                self.sender
                    .send(logs)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                self.buffer.clear();
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            let logs = self.buffer.join("\n");
            self.sender
                .send(logs)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            self.buffer.clear();
        }
        Ok(())
    }
}
