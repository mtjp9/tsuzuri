use crate::{
    event::Envelope,
    projection::{adapter::Adapter, error::Result},
    serde, DomainEvent,
};
use std::marker::PhantomData;

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
    E: DomainEvent,
    EvtSerde: serde::Serde<E>,
{
    pub async fn process_bytes(&self, payload: &[u8]) -> Result<()> {
        let event = self.to_event(payload)?;
        self.adapter.project(event).await?;
        Ok(())
    }

    pub fn to_event(&self, payload: &[u8]) -> Result<Envelope<E>> {
        let event = self.event_serde.deserialize(payload)?;
        let envelope: Envelope<E> = event.into();
        Ok(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event_id::EventIdType,
        message::{self, Metadata},
        projection::{adapter::Projector, error::ProjectionError},
        serde::SerdeError,
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, PartialEq)]
    struct TestEvent {
        pub id: EventIdType,
        pub data: String,
    }

    impl message::Message for TestEvent {
        fn name(&self) -> &'static str {
            "TestEvent"
        }
    }

    impl DomainEvent for TestEvent {
        fn id(&self) -> EventIdType {
            self.id
        }

        fn event_type(&self) -> &'static str {
            "TestEvent"
        }
    }

    #[derive(Clone)]
    struct MockAdapter {
        calls: Arc<Mutex<Vec<TestEvent>>>,
        should_fail: bool,
    }

    impl MockAdapter {
        fn new(should_fail: bool) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                should_fail,
            }
        }

        fn get_calls(&self) -> Vec<TestEvent> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Projector<TestEvent> for MockAdapter {
        async fn project(&self, event: Envelope<TestEvent>) -> Result<()> {
            if self.should_fail {
                return Err(ProjectionError::Database("Mock adapter failed".to_string()));
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

    impl serde::Serializer<TestEvent> for MockSerde {
        fn serialize(&self, _msg: &TestEvent) -> std::result::Result<Vec<u8>, SerdeError> {
            Ok(vec![])
        }
    }

    impl serde::Deserializer<TestEvent> for MockSerde {
        fn deserialize(&self, payload: &[u8]) -> std::result::Result<TestEvent, SerdeError> {
            if self.should_fail {
                return Err(SerdeError::ConversionError("Mock serde failed".to_string()));
            }
            // Simple mock: use payload length as id
            Ok(TestEvent {
                id: EventIdType::new(),
                data: String::from_utf8_lossy(payload).to_string(),
            })
        }
    }

    #[test]
    fn test_processor_creation() {
        let adapter = MockAdapter::new(false);
        let serde = MockSerde::new(false);
        let processor = Processor::<_, TestEvent, _>::new(adapter, serde);

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
            Err(ProjectionError::Serialization(_)) => {}
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
            Err(ProjectionError::Database(msg)) => {
                assert_eq!(msg, "Mock adapter failed");
            }
            _ => panic!("Expected Database error"),
        }
    }

    #[test]
    fn test_to_event() {
        let adapter = MockAdapter::new(false);
        let serde = MockSerde::new(false);
        let processor = Processor::new(adapter, serde);

        let payload = b"test-data";
        let result = processor.to_event(payload);

        assert!(result.is_ok());
        let envelope = result.unwrap();
        assert_eq!(envelope.message.data, "test-data");
        assert_eq!(envelope.metadata, Metadata::default());
    }
}
