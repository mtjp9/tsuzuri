use crate::{
    event::Envelope,
    integration::{adapter::Adapter, error::Result},
    integration_event::IntegrationEvent,
    serde,
};
use std::marker::PhantomData;

/// Integration-specific processor that handles integration events
#[derive(Debug, Clone)]
pub struct Processor<A, E, EvtSerde> {
    pub adapter: A,
    pub event_serde: EvtSerde,
    pub event: PhantomData<E>,
}

impl<A, E, EvtSerde> Processor<A, E, EvtSerde> {
    pub fn new(adapter: A, event_serde: EvtSerde) -> Self {
        Self {
            adapter,
            event_serde,
            event: PhantomData,
        }
    }
}

impl<A, E, EvtSerde> Processor<A, E, EvtSerde>
where
    A: Adapter<E>,
    E: IntegrationEvent,
    EvtSerde: serde::Serde<E>,
{
    pub async fn process_bytes(&self, payload: &[u8]) -> Result<()> {
        let event = self.to_integration_event(payload)?;
        self.adapter.execute(event).await?;
        Ok(())
    }

    pub fn to_integration_event(&self, payload: &[u8]) -> Result<Envelope<E>> {
        let event = self.event_serde.deserialize(payload)?;
        let envelope: Envelope<E> = event.into();
        Ok(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        integration::{adapter::Executer, error::IntegrationError},
        message::{self, Metadata},
        serde::SerdeError,
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, PartialEq)]
    struct TestIntegrationEvent {
        pub id: String,
        pub data: String,
    }

    impl message::Message for TestIntegrationEvent {
        fn name(&self) -> &'static str {
            "TestIntegrationEvent"
        }
    }

    impl IntegrationEvent for TestIntegrationEvent {
        fn id(&self) -> String {
            self.id.clone()
        }

        fn event_type(&self) -> &'static str {
            "TestIntegrationEvent"
        }
    }

    #[derive(Clone)]
    struct MockAdapter {
        calls: Arc<Mutex<Vec<TestIntegrationEvent>>>,
        should_fail: bool,
    }

    impl MockAdapter {
        fn new(should_fail: bool) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                should_fail,
            }
        }

        fn get_calls(&self) -> Vec<TestIntegrationEvent> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Executer<TestIntegrationEvent> for MockAdapter {
        async fn execute(&self, event: Envelope<TestIntegrationEvent>) -> Result<()> {
            if self.should_fail {
                return Err(IntegrationError::Database("Mock adapter failed".to_string()));
            }
            self.calls.lock().unwrap().push(event.message);
            Ok(())
        }
    }

    struct MockSerde {
        should_fail: bool,
    }

    impl MockSerde {
        fn new(should_fail: bool) -> Self {
            Self { should_fail }
        }
    }

    impl serde::Serializer<TestIntegrationEvent> for MockSerde {
        fn serialize(&self, _msg: &TestIntegrationEvent) -> std::result::Result<Vec<u8>, SerdeError> {
            Ok(vec![])
        }
    }

    impl serde::Deserializer<TestIntegrationEvent> for MockSerde {
        fn deserialize(&self, payload: &[u8]) -> std::result::Result<TestIntegrationEvent, SerdeError> {
            if self.should_fail {
                return Err(SerdeError::ConversionError("Mock serde failed".to_string()));
            }
            Ok(TestIntegrationEvent {
                id: format!("event-{}", payload.len()),
                data: String::from_utf8_lossy(payload).to_string(),
            })
        }
    }

    #[test]
    fn test_processor_creation() {
        let adapter = MockAdapter::new(false);
        let serde = MockSerde::new(false);
        let processor = Processor::<_, TestIntegrationEvent, _>::new(adapter, serde);

        // Just verify it creates successfully
        assert!(matches!(processor.event, PhantomData));
    }

    #[tokio::test]
    async fn test_process_bytes_success() {
        let adapter = MockAdapter::new(false);
        let serde = MockSerde::new(false);
        let processor = Processor::new(adapter.clone(), serde);

        let payload = b"test-payload";
        let result = processor.process_bytes(payload).await;

        assert!(result.is_ok());

        let calls = adapter.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].data, "test-payload");
        assert_eq!(calls[0].id, "event-12");
    }

    #[tokio::test]
    async fn test_process_bytes_serde_failure() {
        let adapter = MockAdapter::new(false);
        let serde = MockSerde::new(true); // Will fail
        let processor = Processor::new(adapter.clone(), serde);

        let payload = b"test-payload";
        let result = processor.process_bytes(payload).await;

        assert!(result.is_err());
        match result {
            Err(IntegrationError::Serialization(_)) => {}
            _ => panic!("Expected Serialization error"),
        }

        let calls = adapter.get_calls();
        assert_eq!(calls.len(), 0); // No calls should have been made
    }

    #[tokio::test]
    async fn test_process_bytes_adapter_failure() {
        let adapter = MockAdapter::new(true); // Will fail
        let serde = MockSerde::new(false);
        let processor = Processor::new(adapter.clone(), serde);

        let payload = b"test-payload";
        let result = processor.process_bytes(payload).await;

        assert!(result.is_err());
        match result {
            Err(IntegrationError::Database(msg)) => {
                assert_eq!(msg, "Mock adapter failed");
            }
            _ => panic!("Expected Database error"),
        }
    }

    #[test]
    fn test_to_integration_event() {
        let adapter = MockAdapter::new(false);
        let serde = MockSerde::new(false);
        let processor = Processor::new(adapter, serde);

        let payload = b"test-data";
        let result = processor.to_integration_event(payload);

        assert!(result.is_ok());
        let envelope = result.unwrap();
        assert_eq!(envelope.message.data, "test-data");
        assert_eq!(envelope.message.id, "event-9");
        assert_eq!(envelope.metadata, Metadata::default());
    }
}
