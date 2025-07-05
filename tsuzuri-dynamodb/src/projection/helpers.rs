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
        Some(AttributeValue::B(value)) => {
            // DynamoDB Streams via Kinesis sends binary data in different formats:
            // 1. Sometimes as base64-encoded strings (when coming through Kinesis)
            // 2. Sometimes as raw binary data (when reading directly from DynamoDB)
            // 3. Sometimes as already-decoded JSON bytes (in certain stream configurations)

            // First, check if it's valid UTF-8
            if let Ok(utf8_str) = std::str::from_utf8(value) {
                // If it looks like base64, try to decode it
                if utf8_str
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=')
                    && !utf8_str.is_empty()
                {
                    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, utf8_str) {
                        Ok(decoded) => Ok(decoded),
                        Err(_) => {
                            // Not valid base64, return as-is
                            Ok(value.clone())
                        }
                    }
                } else {
                    // Valid UTF-8 but not base64 (e.g., JSON), return as-is
                    Ok(value.clone())
                }
            } else {
                // Not valid UTF-8, assume it's already decoded binary data
                Ok(value.clone())
            }
        }
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
        // With the new implementation, non-base64 strings are returned as-is
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"not-valid-base64!@#$%");
    }

    #[test]
    fn test_extract_binary_attribute_kinesis_format() {
        let mut attributes = HashMap::new();
        let test_data = b"test binary data";
        let encoded = base64::engine::general_purpose::STANDARD.encode(test_data);
        // Simulate Kinesis format: base64 string as bytes
        attributes.insert("test_field".to_string(), AttributeValue::B(encoded.as_bytes().to_vec()));

        let result = extract_binary_attribute(&attributes, "test_field");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_data);
    }

    #[test]
    fn test_extract_binary_attribute_raw_binary() {
        let mut attributes = HashMap::new();
        // Raw binary data (not base64, not valid UTF-8)
        let test_data = vec![0xFF, 0xFE, 0xFD, 0xFC];
        attributes.insert("test_field".to_string(), AttributeValue::B(test_data.clone()));

        let result = extract_binary_attribute(&attributes, "test_field");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_data);
    }

    #[test]
    fn test_extract_binary_attribute_json_as_bytes() {
        let mut attributes = HashMap::new();
        // JSON data as bytes (like what might come from Kinesis)
        let json_data = b"{}";
        attributes.insert("test_field".to_string(), AttributeValue::B(json_data.to_vec()));

        let result = extract_binary_attribute(&attributes, "test_field");
        // With the new implementation, JSON strings are returned as-is
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"{}");
    }

    #[test]
    fn test_extract_binary_attribute_actual_kinesis_metadata() {
        let mut attributes = HashMap::new();
        // The actual value from the error log
        let metadata_value = b"e30=";
        attributes.insert("metadata".to_string(), AttributeValue::B(metadata_value.to_vec()));

        let result = extract_binary_attribute(&attributes, "metadata");
        assert!(result.is_ok());
        let decoded = result.unwrap();
        // e30= decodes to {}
        assert_eq!(decoded, b"{}");

        // Now test what happens if we get the already-decoded value
        let mut attributes2 = HashMap::new();
        attributes2.insert("metadata".to_string(), AttributeValue::B(b"{}".to_vec()));
        let result2 = extract_binary_attribute(&attributes2, "metadata");
        // With the new implementation, JSON is returned as-is
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), b"{}");
    }
}
