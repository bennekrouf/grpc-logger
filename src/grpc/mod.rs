use serde::Deserialize;

mod layer;
mod writer;

pub use layer::GrpcLayer;

#[derive(Debug, Deserialize)]
pub struct GrpcConfig {
    pub address: String,
    pub port: u16,
}
