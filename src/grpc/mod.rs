use serde::Deserialize;

mod writer;
mod layer;

pub use layer::GrpcLayer;

#[derive(Debug, Deserialize)]
pub struct GrpcConfig {
    pub address: String,
    pub port: u16,
}
