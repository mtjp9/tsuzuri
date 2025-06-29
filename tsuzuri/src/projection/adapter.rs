use crate::{domain_event::DomainEvent, event::Envelope, projection::error::Result};
use async_trait::async_trait;

pub trait Adapter<E>: Projector<E> + Send + Sync + 'static
where
    E: DomainEvent,
{
}

impl<D, E> Adapter<E> for D
where
    D: Projector<E> + Send + Sync + 'static,
    E: DomainEvent,
{
}

#[async_trait]
pub trait Projector<E>: Send + Sync + 'static
where
    E: DomainEvent,
{
    async fn project(&self, event: Envelope<E>) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event_id::EventIdType,
        message::{self, Metadata},
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
    struct MockProjector {
        called: Arc<Mutex<Vec<TestEvent>>>,
        should_fail: bool,
    }

    impl MockProjector {
        fn new(should_fail: bool) -> Self {
            Self {
                called: Arc::new(Mutex::new(Vec::new())),
                should_fail,
            }
        }

        fn get_calls(&self) -> Vec<TestEvent> {
            self.called.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Projector<TestEvent> for MockProjector {
        async fn project(&self, event: Envelope<TestEvent>) -> Result<()> {
            if self.should_fail {
                return Err(crate::projection::error::ProjectionError::Database(
                    "Mock projection failed".to_string(),
                ));
            }
            self.called.lock().unwrap().push(event.message);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_projector_success() {
        let projector = MockProjector::new(false);
        let event = TestEvent {
            id: EventIdType::new(),
            data: "test-data".to_string(),
        };
        let envelope = Envelope {
            message: event.clone(),
            metadata: Metadata::default(),
        };

        let result = projector.project(envelope).await;
        assert!(result.is_ok());

        let calls = projector.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], event);
    }

    #[tokio::test]
    async fn test_projector_failure() {
        let projector = MockProjector::new(true);
        let event = TestEvent {
            id: EventIdType::new(),
            data: "test-data".to_string(),
        };
        let envelope = Envelope {
            message: event,
            metadata: Metadata::default(),
        };

        let result = projector.project(envelope).await;
        assert!(result.is_err());
        match result {
            Err(crate::projection::error::ProjectionError::Database(msg)) => {
                assert_eq!(msg, "Mock projection failed");
            }
            _ => panic!("Expected Database error"),
        }

        let calls = projector.get_calls();
        assert_eq!(calls.len(), 0);
    }

    #[test]
    fn test_adapter_trait_impl() {
        let projector = MockProjector::new(false);

        fn assert_adapter<T: Adapter<TestEvent>>(_: &T) {}

        assert_adapter(&projector);
    }
}
