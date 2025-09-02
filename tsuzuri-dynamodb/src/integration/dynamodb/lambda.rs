use crate::error::{Result, StreamProcessorError};
use crate::integration::event_type_router::ProcessorBasedEventRouter;
use crate::integration::helpers::{extract_binary_attribute, extract_string_attribute};
use aws_lambda_events::dynamodb::Event as DynamoDbStreamsEvent;
use lambda_runtime::LambdaEvent;

/// Process DynamoDB Streams events (Lambda trigger) for Integration routing
pub async fn process_event(
    router: &mut ProcessorBasedEventRouter,
    event: LambdaEvent<DynamoDbStreamsEvent>,
) -> Result<()> {
    for record in event.payload.records {
        process_single_record(router, &record).await?;
    }
    Ok(())
}

async fn process_single_record(
    router: &mut ProcessorBasedEventRouter,
    record: &aws_lambda_events::dynamodb::EventRecord,
) -> Result<()> {
    // Only process INSERT and MODIFY events which have new images
    let event_name = &record.event_name;
    if event_name != "INSERT" && event_name != "MODIFY" {
        return Ok(());
    }

    let stream_record = &record.change;
    let attribute_values = stream_record.new_image.clone().into_inner();

    let event_type = extract_string_attribute(&attribute_values, "event_type")?;
    let payload_bytes = extract_binary_attribute(&attribute_values, "payload")?;

    router
        .process_bytes(event_type, &payload_bytes)
        .await
        .map_err(|e| StreamProcessorError::InvalidData(format!("Failed to process event: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use aws_lambda_events::dynamodb::{EventRecord, StreamRecord, StreamViewType};
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
        async fn process_bytes(&mut self, payload: &[u8]) -> IntegrationResult<()> {
            if self.should_fail {
                return Err(tsuzuri::integration::error::IntegrationError::Database(
                    "Mock error".to_string(),
                ));
            }
            let mut calls = self.calls.lock().unwrap();
            calls.push(("event_type".to_string(), payload.to_vec()));
            Ok(())
        }
    }

    fn create_test_lambda_event(records: Vec<EventRecord>) -> LambdaEvent<DynamoDbStreamsEvent> {
        let event = DynamoDbStreamsEvent { records };
        let context = Context::default();
        LambdaEvent::new(event, context)
    }

    fn create_dynamodb_event_record(event_name: &str, event_type: &str, payload: &[u8]) -> EventRecord {
        let mut new_image = HashMap::new();
        new_image.insert("event_type".to_string(), AttributeValue::S(event_type.to_string()));
        new_image.insert(
            "payload".to_string(),
            AttributeValue::B(base64::engine::general_purpose::STANDARD.encode(payload).into_bytes()),
        );

        EventRecord {
            aws_region: "us-east-1".to_string(),
            change: StreamRecord {
                approximate_creation_date_time: Utc::now(),
                keys: serde_dynamo::Item::from(HashMap::new()),
                new_image: new_image.into(),
                old_image: serde_dynamo::Item::from(HashMap::new()),
                sequence_number: Some("12345".to_string()),
                size_bytes: 1024,
                stream_view_type: Some(StreamViewType::NewAndOldImages),
            },
            event_id: "test-event-id".to_string(),
            event_name: event_name.to_string(),
            event_source: Some("aws:dynamodb".to_string()),
            event_source_arn: Some("arn:aws:dynamodb:us-east-1:123456789012:table/test".to_string()),
            user_identity: None,
            event_version: Some("1.0".to_string()),
            record_format: None,
            table_name: Some("test-table".to_string()),
        }
    }

    #[tokio::test]
    async fn test_process_single_record_insert_success() {
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

        let mut router = ProcessorBasedEventRouter { routes };

        let record = create_dynamodb_event_record("INSERT", "TestEvent", b"test payload");
        let result = process_single_record(&mut router, &record).await;
        assert!(result.is_ok());

        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, b"test payload");
    }

    #[tokio::test]
    async fn test_process_single_record_modify_success() {
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

        let mut router = ProcessorBasedEventRouter { routes };

        let record = create_dynamodb_event_record("MODIFY", "TestEvent", b"test payload");
        let result = process_single_record(&mut router, &record).await;
        assert!(result.is_ok());

        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
    }

    #[tokio::test]
    async fn test_process_single_record_remove_skipped() {
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

        let mut router = ProcessorBasedEventRouter { routes };

        let record = create_dynamodb_event_record("REMOVE", "TestEvent", b"test payload");
        let result = process_single_record(&mut router, &record).await;
        assert!(result.is_ok());

        // Not called for REMOVE events
        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 0);
    }

    #[tokio::test]
    async fn test_process_dynamodb_lambda_event_success() {
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

        let mut router = ProcessorBasedEventRouter { routes };

        let record1 = create_dynamodb_event_record("INSERT", "TestEvent", b"payload1");
        let record2 = create_dynamodb_event_record("MODIFY", "TestEvent", b"payload2");
        let record3 = create_dynamodb_event_record("REMOVE", "TestEvent", b"payload3");
        let lambda_event = create_test_lambda_event(vec![record1, record2, record3]);

        let result = process_event(&mut router, lambda_event).await;
        assert!(result.is_ok());

        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, b"payload1");
        assert_eq!(calls[1].1, b"payload2");
    }

    #[tokio::test]
    async fn test_process_dynamodb_lambda_event_with_error() {
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

        let mut router = ProcessorBasedEventRouter { routes };

        let record = create_dynamodb_event_record("INSERT", "TestEvent", b"payload");
        let lambda_event = create_test_lambda_event(vec![record]);

        let result = process_event(&mut router, lambda_event).await;
        assert!(result.is_err());
    }
}
