use tsuzuri::{integration::error::IntegrationError, projection::error::ProjectionError};

#[derive(thiserror::Error, Debug)]
pub enum StreamProcessorError {
    #[error("Kinesis Data Streams error: {0}")]
    KinesisDataStreams(String),

    #[error("Tsuzuri projection error: {0}")]
    Projection(#[from] ProjectionError),

    #[error("Tsuzuri integration error: {0}")]
    Integration(#[from] IntegrationError),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

pub type Result<T> = std::result::Result<T, StreamProcessorError>;
