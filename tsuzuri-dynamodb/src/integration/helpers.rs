use crate::error::{Result, StreamProcessorError};
use serde_dynamo::AttributeValue;
use std::collections::HashMap;

pub fn extract_string_attribute<'a>(
    attributes: &'a HashMap<String, AttributeValue>,
    field_name: &str,
) -> Result<&'a str> {
    match attributes.get(field_name) {
        Some(AttributeValue::S(value)) => Ok(value.as_str()),
        Some(_) => Err(StreamProcessorError::InvalidData(format!(
            "Field '{field_name}' is not a string"
        ))),
        None => Err(StreamProcessorError::InvalidData(format!(
            "Missing required field '{field_name}'"
        ))),
    }
}

pub fn extract_binary_attribute(attributes: &HashMap<String, AttributeValue>, field_name: &str) -> Result<Vec<u8>> {
    match attributes.get(field_name) {
        Some(AttributeValue::B(bytes)) => base64::Engine::decode(&base64::engine::general_purpose::STANDARD, bytes)
            .map_err(|e| {
                StreamProcessorError::InvalidData(format!("Failed to decode {field_name} as base64: {e}"))
            }),
        Some(_) => Err(StreamProcessorError::InvalidData(format!(
            "Field '{field_name}' is not binary data"
        ))),
        None => Err(StreamProcessorError::InvalidData(format!(
            "Missing required field '{field_name}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn test_extract_string_attribute_success() {
        let mut attributes = HashMap::new();
        attributes.insert("test_field".to_string(), AttributeValue::S("test_value".to_string()));

        let result = extract_string_attribute(&attributes, "test_field");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_value");
    }

    #[test]
    fn test_extract_string_attribute_missing_field() {
        let attributes = HashMap::new();

        let result = extract_string_attribute(&attributes, "missing_field");
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert_eq!(msg, "Missing required field 'missing_field'");
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_extract_string_attribute_wrong_type() {
        let mut attributes = HashMap::new();
        attributes.insert("test_field".to_string(), AttributeValue::N("123".to_string()));

        let result = extract_string_attribute(&attributes, "test_field");
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert_eq!(msg, "Field 'test_field' is not a string");
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_extract_binary_attribute_success() {
        let mut attributes = HashMap::new();
        let test_data = b"test binary data";
        let encoded = base64::engine::general_purpose::STANDARD.encode(test_data);
        attributes.insert("test_field".to_string(), AttributeValue::B(encoded.into_bytes()));

        let result = extract_binary_attribute(&attributes, "test_field");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_data);
    }

    #[test]
    fn test_extract_binary_attribute_missing_field() {
        let attributes = HashMap::new();

        let result = extract_binary_attribute(&attributes, "missing_field");
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert_eq!(msg, "Missing required field 'missing_field'");
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_extract_binary_attribute_wrong_type() {
        let mut attributes = HashMap::new();
        attributes.insert("test_field".to_string(), AttributeValue::S("not binary".to_string()));

        let result = extract_binary_attribute(&attributes, "test_field");
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert_eq!(msg, "Field 'test_field' is not binary data");
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_extract_binary_attribute_invalid_base64() {
        let mut attributes = HashMap::new();
        // Invalid base64 string
        attributes.insert(
            "test_field".to_string(),
            AttributeValue::B(b"not-valid-base64!@#$%".to_vec()),
        );

        let result = extract_binary_attribute(&attributes, "test_field");
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert!(msg.starts_with("Failed to decode test_field as base64:"));
            }
            _ => panic!("Expected InvalidData error"),
        }
    }
}
