use crate::{event::Envelope, integration::error::Result, integration_event::IntegrationEvent};
use async_trait::async_trait;

pub trait Adapter<E: IntegrationEvent>: Executer<E> + Send + Sync + 'static {}

impl<D, E> Adapter<E> for D
where
    D: Executer<E> + Send + Sync + 'static,
    E: IntegrationEvent,
{
}

#[async_trait]
pub trait Executer<E: IntegrationEvent>: Send + Sync + 'static {
    async fn execute(&self, event: Envelope<E>) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{self, Metadata};
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
    struct MockExecuter {
        called: Arc<Mutex<Vec<TestIntegrationEvent>>>,
        should_fail: bool,
    }

    impl MockExecuter {
        fn new(should_fail: bool) -> Self {
            Self {
                called: Arc::new(Mutex::new(Vec::new())),
                should_fail,
            }
        }

        fn get_calls(&self) -> Vec<TestIntegrationEvent> {
            self.called.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Executer<TestIntegrationEvent> for MockExecuter {
        async fn execute(&self, event: Envelope<TestIntegrationEvent>) -> Result<()> {
            if self.should_fail {
                return Err(crate::integration::error::IntegrationError::Database(
                    "Mock execution failed".to_string(),
                ));
            }
            self.called.lock().unwrap().push(event.message);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_executer_success() {
        let executer = MockExecuter::new(false);
        let event = TestIntegrationEvent {
            id: "test-id".to_string(),
            data: "test-data".to_string(),
        };
        let envelope = Envelope {
            message: event.clone(),
            metadata: Metadata::default(),
        };

        let result = executer.execute(envelope).await;
        assert!(result.is_ok());

        let calls = executer.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], event);
    }

    #[tokio::test]
    async fn test_executer_failure() {
        let executer = MockExecuter::new(true);
        let event = TestIntegrationEvent {
            id: "test-id".to_string(),
            data: "test-data".to_string(),
        };
        let envelope = Envelope {
            message: event,
            metadata: Metadata::default(),
        };

        let result = executer.execute(envelope).await;
        assert!(result.is_err());
        match result {
            Err(crate::integration::error::IntegrationError::Database(msg)) => {
                assert_eq!(msg, "Mock execution failed");
            }
            _ => panic!("Expected Database error"),
        }

        let calls = executer.get_calls();
        assert_eq!(calls.len(), 0);
    }

    #[test]
    fn test_adapter_trait_impl() {
        let executer = MockExecuter::new(false);

        fn assert_adapter<T: Adapter<TestIntegrationEvent>>(_: &T) {}

        assert_adapter(&executer);
    }
}
