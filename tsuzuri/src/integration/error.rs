use crate::serde::SerdeError;

#[derive(thiserror::Error, Debug)]
pub enum IntegrationError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerdeError),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Stream processing error: {0}")]
    StreamProcessing(String),
    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, IntegrationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let db_error = IntegrationError::Database("Connection failed".to_string());
        assert_eq!(db_error.to_string(), "Database error: Connection failed");

        let invalid_data_error = IntegrationError::InvalidData("Bad format".to_string());
        assert_eq!(invalid_data_error.to_string(), "Invalid data: Bad format");

        let stream_error = IntegrationError::StreamProcessing("Stream closed".to_string());
        assert_eq!(stream_error.to_string(), "Stream processing error: Stream closed");
    }

    #[test]
    fn test_serde_error_conversion() {
        let serde_error = SerdeError::ConversionError("Type mismatch".to_string());
        let integration_error: IntegrationError = serde_error.into();
        assert!(matches!(integration_error, IntegrationError::Serialization(_)));
        assert_eq!(
            integration_error.to_string(),
            "Serialization error: failed to convert type values: Type mismatch"
        );
    }

    #[test]
    fn test_json_error_conversion() {
        let json_str = "{ invalid json }";
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let integration_error: IntegrationError = json_error.into();
        assert!(matches!(integration_error, IntegrationError::Json(_)));
        assert!(integration_error.to_string().starts_with("Json error:"));
    }

    #[test]
    fn test_result_type() {
        fn returns_result() -> Result<String> {
            Ok("success".to_string())
        }

        let result = returns_result();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        fn returns_error() -> Result<String> {
            Err(IntegrationError::Database("Failed".to_string()))
        }

        let error = returns_error();
        assert!(error.is_err());
    }
}
