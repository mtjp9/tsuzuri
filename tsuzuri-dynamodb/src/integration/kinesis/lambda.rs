use crate::error::{Result, StreamProcessorError};
use crate::integration::event_type_router::ProcessorBasedEventRouter;
use crate::integration::helpers::{extract_binary_attribute, extract_string_attribute};
use aws_lambda_events::dynamodb::StreamRecord;
use aws_lambda_events::kinesis::KinesisEvent;
use lambda_runtime::LambdaEvent;

pub async fn process_kinesis_lambda_event(
    router: &ProcessorBasedEventRouter,
    event: LambdaEvent<KinesisEvent>,
) -> Result<()> {
    for record in event.payload.records {
        process_single_record(router, &record.kinesis.data).await?;
    }
    Ok(())
}

async fn process_single_record(router: &ProcessorBasedEventRouter, data: &[u8]) -> Result<()> {
    let stream_record = extract_stream_record(data)?;
    let attribute_values = stream_record.new_image.into_inner();

    let event_type = extract_string_attribute(&attribute_values, "event_type")?;
    let payload_bytes = extract_binary_attribute(&attribute_values, "payload")?;

    router
        .process_bytes(event_type, &payload_bytes)
        .await
        .map_err(|e| StreamProcessorError::InvalidData(format!("Failed to process event: {e}")))
}

fn extract_stream_record(data: &[u8]) -> Result<StreamRecord> {
    let json: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| StreamProcessorError::InvalidData(format!("Failed to deserialize Kinesis data: {e}")))?;

    let dynamodb_data = json
        .get("dynamodb")
        .ok_or_else(|| StreamProcessorError::InvalidData("Missing 'dynamodb' field in Kinesis record".to_string()))?;

    serde_json::from_value(dynamodb_data.clone())
        .map_err(|e| StreamProcessorError::InvalidData(format!("Failed to parse DynamoDB stream record: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use aws_lambda_events::{
        dynamodb::{StreamRecord, StreamViewType},
        encodings::{Base64Data, SecondTimestamp},
        kinesis::{KinesisEventRecord, KinesisRecord},
    };
    use base64::Engine;
    use chrono::Utc;
    use lambda_runtime::Context;
    use serde_dynamo::AttributeValue;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tsuzuri::integration::error::Result as IntegrationResult;

    // Mock ProcessorTrait implementation for testing
    type MockProcessorCalls = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

    struct MockProcessor {
        calls: MockProcessorCalls,
        should_fail: bool,
    }

    #[async_trait]
    impl crate::integration::event_type_router::ProcessorTrait for Arc<MockProcessor> {
        async fn process_bytes(&self, payload: &[u8]) -> IntegrationResult<()> {
            if self.should_fail {
                return Err(tsuzuri::integration::error::IntegrationError::Database(
                    "Mock error".to_string(),
                ));
            }
            // Store the call for verification
            let mut calls = self.calls.lock().unwrap();
            calls.push(("event_type".to_string(), payload.to_vec()));
            Ok(())
        }
    }

    fn create_test_lambda_event(records: Vec<KinesisEventRecord>) -> LambdaEvent<KinesisEvent> {
        let event = KinesisEvent { records };
        let context = Context::default();
        LambdaEvent::new(event, context)
    }

    fn create_kinesis_record(data: Vec<u8>) -> KinesisEventRecord {
        KinesisEventRecord {
            aws_region: Some("us-east-1".to_string()),
            event_id: Some("test-event-id".to_string()),
            event_name: Some("aws:kinesis:record".to_string()),
            event_source: Some("aws:kinesis".to_string()),
            event_version: Some("1.0".to_string()),
            event_source_arn: Some("arn:aws:kinesis:us-east-1:123456789012:stream/test".to_string()),
            invoke_identity_arn: Some("arn:aws:iam::123456789012:role/test".to_string()),
            kinesis: KinesisRecord {
                approximate_arrival_timestamp: SecondTimestamp(Utc::now()),
                data: Base64Data(data),
                encryption_type: aws_lambda_events::kinesis::KinesisEncryptionType::None,
                partition_key: "test-partition".to_string(),
                sequence_number: "12345".to_string(),
                kinesis_schema_version: Some("1.0".to_string()),
            },
        }
    }

    fn create_dynamodb_stream_data(event_type: &str, payload: &[u8]) -> Vec<u8> {
        let mut new_image = HashMap::new();
        new_image.insert("event_type".to_string(), AttributeValue::S(event_type.to_string()));
        new_image.insert(
            "payload".to_string(),
            AttributeValue::B(base64::engine::general_purpose::STANDARD.encode(payload).into_bytes()),
        );

        let stream_record = StreamRecord {
            approximate_creation_date_time: Utc::now(),
            keys: serde_dynamo::Item::from(HashMap::new()),
            new_image: new_image.into(),
            old_image: serde_dynamo::Item::from(HashMap::new()),
            sequence_number: Some("12345".to_string()),
            size_bytes: 1024,
            stream_view_type: Some(StreamViewType::NewAndOldImages),
        };

        let wrapper = serde_json::json!({
            "dynamodb": stream_record,
        });

        serde_json::to_vec(&wrapper).unwrap()
    }

    #[test]
    fn test_extract_stream_record_success() {
        let stream_data = create_dynamodb_stream_data("TestEvent", b"test payload");

        let result = extract_stream_record(&stream_data);
        assert!(result.is_ok());

        let record = result.unwrap();
        // Check that new_image has data
        let new_image_json = serde_json::to_value(&record.new_image).unwrap();
        assert!(new_image_json.is_object());
    }

    #[test]
    fn test_extract_stream_record_invalid_json() {
        let invalid_data = b"not json";

        let result = extract_stream_record(invalid_data);
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert!(msg.starts_with("Failed to deserialize Kinesis data:"));
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[test]
    fn test_extract_stream_record_missing_dynamodb_field() {
        let data_without_dynamodb = serde_json::json!({
            "other_field": "value"
        });
        let data = serde_json::to_vec(&data_without_dynamodb).unwrap();

        let result = extract_stream_record(&data);
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert_eq!(msg, "Missing 'dynamodb' field in Kinesis record");
            }
            _ => panic!("Expected InvalidData error"),
        }
    }

    #[tokio::test]
    async fn test_process_single_record_success() {
        let mock_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        });

        let mut routes: HashMap<String, Box<dyn crate::integration::event_type_router::ProcessorTrait>> =
            HashMap::new();
        routes.insert(
            "TestEvent".to_string(),
            Box::new(mock_processor.clone()) as Box<dyn crate::integration::event_type_router::ProcessorTrait>,
        );

        let router = ProcessorBasedEventRouter { routes };

        let stream_data = create_dynamodb_stream_data("TestEvent", b"test payload");

        let result = process_single_record(&router, &stream_data).await;
        assert!(result.is_ok());

        // Verify the mock was called
        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, b"test payload");
    }

    #[tokio::test]
    async fn test_process_kinesis_lambda_event_success() {
        let mock_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        });

        let mut routes: HashMap<String, Box<dyn crate::integration::event_type_router::ProcessorTrait>> =
            HashMap::new();
        routes.insert(
            "TestEvent".to_string(),
            Box::new(mock_processor.clone()) as Box<dyn crate::integration::event_type_router::ProcessorTrait>,
        );

        let router = ProcessorBasedEventRouter { routes };

        // Create test data
        let stream_data1 = create_dynamodb_stream_data("TestEvent", b"payload1");
        let stream_data2 = create_dynamodb_stream_data("TestEvent", b"payload2");

        let records = vec![create_kinesis_record(stream_data1), create_kinesis_record(stream_data2)];

        let lambda_event = create_test_lambda_event(records);

        let result = process_kinesis_lambda_event(&router, lambda_event).await;
        assert!(result.is_ok());

        // Verify both records were processed
        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, b"payload1");
        assert_eq!(calls[1].1, b"payload2");
    }

    #[tokio::test]
    async fn test_process_kinesis_lambda_event_with_error() {
        let mock_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        });

        let mut routes: HashMap<String, Box<dyn crate::integration::event_type_router::ProcessorTrait>> =
            HashMap::new();
        routes.insert(
            "TestEvent".to_string(),
            Box::new(mock_processor) as Box<dyn crate::integration::event_type_router::ProcessorTrait>,
        );

        let router = ProcessorBasedEventRouter { routes };

        let stream_data = create_dynamodb_stream_data("TestEvent", b"payload");
        let records = vec![create_kinesis_record(stream_data)];
        let lambda_event = create_test_lambda_event(records);

        let result = process_kinesis_lambda_event(&router, lambda_event).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_single_record_missing_event_type() {
        let _mock_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        });

        let routes: HashMap<String, Box<dyn crate::integration::event_type_router::ProcessorTrait>> = HashMap::new();
        let router = ProcessorBasedEventRouter { routes };

        // Create stream data without event_type field
        let mut new_image = HashMap::new();
        new_image.insert(
            "payload".to_string(),
            AttributeValue::B(base64::engine::general_purpose::STANDARD.encode(b"test").into_bytes()),
        );

        let stream_record = StreamRecord {
            approximate_creation_date_time: Utc::now(),
            keys: serde_dynamo::Item::from(HashMap::new()),
            new_image: new_image.into(),
            old_image: serde_dynamo::Item::from(HashMap::new()),
            sequence_number: Some("12345".to_string()),
            size_bytes: 1024,
            stream_view_type: Some(StreamViewType::NewAndOldImages),
        };

        let wrapper = serde_json::json!({
            "dynamodb": stream_record,
        });

        let stream_data = serde_json::to_vec(&wrapper).unwrap();

        let result = process_single_record(&router, &stream_data).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            StreamProcessorError::InvalidData(msg) => {
                assert_eq!(msg, "Missing required field 'event_type'");
            }
            _ => panic!("Expected InvalidData error"),
        }
    }
}
