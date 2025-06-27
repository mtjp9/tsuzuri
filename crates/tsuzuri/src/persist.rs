use crate::{error::AggregateError, serde};
use std::error;

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("optimistic lock error")]
    OptimisticLockError,
    #[error("{0}")]
    ConnectionError(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("{0}")]
    DeserializationError(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("{0}")]
    UnknownError(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl<T: std::error::Error> From<PersistenceError> for AggregateError<T> {
    fn from(err: PersistenceError) -> Self {
        match err {
            PersistenceError::OptimisticLockError => Self::AggregateConflict,
            PersistenceError::ConnectionError(error) => Self::DatabaseConnectionError(error),
            PersistenceError::DeserializationError(error) => Self::DeserializationError(error),
            PersistenceError::UnknownError(error) => Self::UnexpectedError(error),
        }
    }
}

/*
 *
 * serde_json::Error, serde::error::Error
 *
 * re-export
 *
 */
impl From<serde_json::Error> for PersistenceError {
    fn from(err: serde_json::Error) -> Self {
        match err.classify() {
            serde_json::error::Category::Data | serde_json::error::Category::Syntax => {
                Self::DeserializationError(Box::new(err))
            }
            serde_json::error::Category::Io | serde_json::error::Category::Eof => {
                Self::UnknownError(Box::new(err))
            }
        }
    }
}

impl<T: error::Error> From<serde_json::error::Error> for AggregateError<T> {
    fn from(err: serde_json::error::Error) -> Self {
        match err.classify() {
            serde_json::error::Category::Data | serde_json::error::Category::Syntax => {
                Self::DeserializationError(Box::new(err))
            }
            serde_json::error::Category::Io | serde_json::error::Category::Eof => {
                Self::UnexpectedError(Box::new(err))
            }
        }
    }
}

/*
 *
 * serde::SerdeError
 *
 */
impl From<serde::SerdeError> for PersistenceError {
    fn from(err: serde::SerdeError) -> Self {
        match err {
            serde::SerdeError::ConversionError(msg) => Self::DeserializationError(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, msg),
            )),
            serde::SerdeError::JsonError(err) => Self::DeserializationError(Box::new(err)),
            serde::SerdeError::ProtobufDeserializationError(err) => {
                Self::DeserializationError(Box::new(err))
            }
        }
    }
}

impl<T: error::Error> From<serde::SerdeError> for AggregateError<T> {
    fn from(err: serde::SerdeError) -> Self {
        match err {
            serde::SerdeError::ConversionError(msg) => Self::DeserializationError(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, msg),
            )),
            serde::SerdeError::JsonError(err) => Self::DeserializationError(Box::new(err)),
            serde::SerdeError::ProtobufDeserializationError(err) => {
                Self::DeserializationError(Box::new(err))
            }
        }
    }
}
