use async_trait::async_trait;
use std::collections::HashMap;
use tsuzuri::{
    event::Envelope,
    integration::{
        adapter::{Adapter, Executer},
        error::Result,
        processor::Processor,
    },
    integration_event::IntegrationEvent,
    serde::Serde,
};

/// Type-safe event router with deserialization for integration events
pub struct TypedEventRouter<E> {
    routes: HashMap<String, Box<dyn Executer<E>>>,
    _phantom: std::marker::PhantomData<E>,
}

impl<E> TypedEventRouter<E>
where
    E: IntegrationEvent,
{
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn routes(&self) -> &HashMap<String, Box<dyn Executer<E>>> {
        &self.routes
    }

    pub fn route(mut self, event_name: &str, integrater: Box<dyn Executer<E>>) -> Self {
        self.routes.insert(event_name.to_string(), integrater);
        self
    }
}

impl<E> Default for TypedEventRouter<E>
where
    E: IntegrationEvent,
{
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<E> Executer<E> for TypedEventRouter<E>
where
    E: IntegrationEvent + Send + Sync,
{
    async fn execute(&self, event: Envelope<E>) -> Result<()> {
        // Extract event name from the envelope message
        let event_name = event.message.name();

        // Find the appropriate executer
        match self.routes.get(event_name) {
            Some(executer) => executer.execute(event).await,
            None => Ok(()),
        }
    }
}

/// Processor-based event router that can handle payload/metadata directly
/// This router can handle multiple different event types
pub struct ProcessorBasedEventRouter {
    pub(crate) routes: HashMap<String, Box<dyn ProcessorTrait>>,
}

/// Trait to abstract over different processor types
#[async_trait]
pub trait ProcessorTrait: Send + Sync {
    async fn process_bytes(&self, payload: &[u8]) -> Result<()>;
}

impl ProcessorBasedEventRouter {
    pub fn new() -> Self {
        Self { routes: HashMap::new() }
    }

    /// Register a processor for an event type prefix
    /// Example: registering "ProjectIntegrationEvent" will match "ProjectIntegrationEventBodyChanged"
    pub fn route_processor<A, E, EvtSerde>(mut self, event_prefix: &str, processor: Processor<A, E, EvtSerde>) -> Self
    where
        A: Adapter<E> + 'static,
        E: IntegrationEvent + 'static,
        EvtSerde: Serde<E> + 'static,
    {
        self.routes
            .insert(event_prefix.to_string(), Box::new(ProcessorWrapper { processor }));
        self
    }

    /// Process bytes through appropriate processor
    /// Each processor will handle its own deserialization using its own Serde implementation
    /// Uses prefix matching: "ProjectIntegrationEvent" matches "ProjectIntegrationEventBodyChanged"
    pub async fn process_bytes(&self, event_name: &str, payload: &[u8]) -> Result<()> {
        // First try exact match
        if let Some(processor) = self.routes.get(event_name) {
            return processor.process_bytes(payload).await;
        }

        // Then try prefix match
        for (registered_prefix, processor) in &self.routes {
            if event_name.starts_with(registered_prefix) {
                return processor.process_bytes(payload).await;
            }
        }

        Ok(())
    }
}

impl Default for ProcessorBasedEventRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper to make Processor implement ProcessorTrait
struct ProcessorWrapper<A, E, EvtSerde> {
    processor: Processor<A, E, EvtSerde>,
}

#[async_trait]
impl<A, E, EvtSerde> ProcessorTrait for ProcessorWrapper<A, E, EvtSerde>
where
    A: Adapter<E> + Send + Sync,
    E: IntegrationEvent + Send + Sync,
    EvtSerde: Serde<E> + Send + Sync,
{
    async fn process_bytes(&self, payload: &[u8]) -> Result<()> {
        self.processor.process_bytes(payload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tsuzuri::{
        event::Metadata, integration::error::IntegrationError, integration_event::IntegrationEvent, message::Message,
    };

    #[derive(Debug, Clone, PartialEq)]
    struct TestIntegrationEvent {
        pub id: String,
        pub data: String,
    }

    impl Message for TestIntegrationEvent {
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

    #[allow(dead_code)]
    #[derive(Debug, Clone, PartialEq)]
    struct AnotherTestEvent {
        pub id: String,
        pub value: i32,
    }

    impl Message for AnotherTestEvent {
        fn name(&self) -> &'static str {
            "AnotherTestEvent"
        }
    }

    impl IntegrationEvent for AnotherTestEvent {
        fn id(&self) -> String {
            self.id.clone()
        }

        fn event_type(&self) -> &'static str {
            "AnotherTestEvent"
        }
    }

    // Mock Executer for testing TypedEventRouter
    struct MockExecuter<E: Message> {
        calls: Arc<Mutex<Vec<Envelope<E>>>>,
        should_fail: bool,
    }

    impl<E: Message> MockExecuter<E> {
        fn new(should_fail: bool) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                should_fail,
            }
        }

        #[allow(dead_code)]
        fn get_calls(&self) -> Vec<Envelope<E>>
        where
            E: Clone,
        {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl<E> Executer<E> for MockExecuter<E>
    where
        E: IntegrationEvent + Send + Sync + Clone,
    {
        async fn execute(&self, event: Envelope<E>) -> Result<()> {
            if self.should_fail {
                return Err(IntegrationError::Database("Mock executer failed".to_string()));
            }
            self.calls.lock().unwrap().push(event);
            Ok(())
        }
    }

    // Mock ProcessorTrait for testing ProcessorBasedEventRouter
    type MockProcessorCalls = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

    struct MockProcessor {
        calls: MockProcessorCalls,
        should_fail: bool,
    }

    #[async_trait]
    impl ProcessorTrait for Arc<MockProcessor> {
        async fn process_bytes(&self, payload: &[u8]) -> Result<()> {
            if self.should_fail {
                return Err(IntegrationError::Database("Mock processor failed".to_string()));
            }
            self.calls.lock().unwrap().push(("event".to_string(), payload.to_vec()));
            Ok(())
        }
    }

    #[test]
    fn test_typed_event_router_creation() {
        let router: TypedEventRouter<TestIntegrationEvent> = TypedEventRouter::new();
        assert_eq!(router.routes().len(), 0);
    }

    #[test]
    fn test_typed_event_router_default() {
        let router: TypedEventRouter<TestIntegrationEvent> = TypedEventRouter::default();
        assert_eq!(router.routes().len(), 0);
    }

    #[test]
    fn test_typed_event_router_route_registration() {
        let executer = MockExecuter::<TestIntegrationEvent>::new(false);
        let router = TypedEventRouter::new().route("TestIntegrationEvent", Box::new(executer));

        assert_eq!(router.routes().len(), 1);
        assert!(router.routes().contains_key("TestIntegrationEvent"));
    }

    #[tokio::test]
    async fn test_typed_event_router_execute_registered_event() {
        let executer = MockExecuter::<TestIntegrationEvent>::new(false);
        let router = TypedEventRouter::new().route("TestIntegrationEvent", Box::new(executer));

        let event = TestIntegrationEvent {
            id: "test-id".to_string(),
            data: "test data".to_string(),
        };
        let envelope = Envelope::from(event).set_metadata(Metadata::default());

        let result = router.execute(envelope.clone()).await;
        assert!(result.is_ok());

        // Since executer is moved, we need to check the result
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_typed_event_router_execute_unregistered_event() {
        let router: TypedEventRouter<TestIntegrationEvent> = TypedEventRouter::new();

        let event = TestIntegrationEvent {
            id: "test-id".to_string(),
            data: "test data".to_string(),
        };
        let envelope = Envelope::from(event);

        // Should return Ok(()) for unregistered events
        let result = router.execute(envelope).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_typed_event_router_execute_with_failure() {
        let executer = MockExecuter::<TestIntegrationEvent>::new(true);
        let router = TypedEventRouter::new().route("TestIntegrationEvent", Box::new(executer));

        let event = TestIntegrationEvent {
            id: "test-id".to_string(),
            data: "test data".to_string(),
        };
        let envelope = Envelope::from(event);

        let result = router.execute(envelope).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            IntegrationError::Database(msg) => assert_eq!(msg, "Mock executer failed"),
            _ => panic!("Expected Database error"),
        }
    }

    #[test]
    fn test_processor_based_event_router_creation() {
        let router = ProcessorBasedEventRouter::new();
        assert_eq!(router.routes.len(), 0);
    }

    #[test]
    fn test_processor_based_event_router_default() {
        let router = ProcessorBasedEventRouter::default();
        assert_eq!(router.routes.len(), 0);
    }

    #[tokio::test]
    async fn test_processor_based_event_router_exact_match() {
        let mock_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        });

        let mut routes: HashMap<String, Box<dyn ProcessorTrait>> = HashMap::new();
        routes.insert(
            "TestEvent".to_string(),
            Box::new(mock_processor.clone()) as Box<dyn ProcessorTrait>,
        );

        let router = ProcessorBasedEventRouter { routes };

        let payload = b"test payload";
        let result = router.process_bytes("TestEvent", payload).await;
        assert!(result.is_ok());

        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, payload.to_vec());
    }

    #[tokio::test]
    async fn test_processor_based_event_router_prefix_match() {
        let mock_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        });

        let mut routes: HashMap<String, Box<dyn ProcessorTrait>> = HashMap::new();
        routes.insert(
            "ProjectIntegrationEvent".to_string(),
            Box::new(mock_processor.clone()) as Box<dyn ProcessorTrait>,
        );

        let router = ProcessorBasedEventRouter { routes };

        let payload = b"test payload";
        let result = router
            .process_bytes("ProjectIntegrationEventBodyChanged", payload)
            .await;
        assert!(result.is_ok());

        let calls = mock_processor.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, payload.to_vec());
    }

    #[tokio::test]
    async fn test_processor_based_event_router_no_match() {
        let router = ProcessorBasedEventRouter::new();

        let payload = b"test payload";
        let result = router.process_bytes("UnknownEvent", payload).await;
        // Should return Ok(()) for unmatched events
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_processor_based_event_router_with_failure() {
        let mock_processor = MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        };

        let mut routes: HashMap<String, Box<dyn ProcessorTrait>> = HashMap::new();
        routes.insert(
            "TestEvent".to_string(),
            Box::new(Arc::new(mock_processor)) as Box<dyn ProcessorTrait>,
        );

        let router = ProcessorBasedEventRouter { routes };

        let payload = b"test payload";
        let result = router.process_bytes("TestEvent", payload).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            IntegrationError::Database(msg) => assert_eq!(msg, "Mock processor failed"),
            _ => panic!("Expected Database error"),
        }
    }

    #[tokio::test]
    async fn test_processor_based_event_router_exact_match_takes_precedence() {
        let exact_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        });

        let prefix_processor = Arc::new(MockProcessor {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        });

        let mut routes: HashMap<String, Box<dyn ProcessorTrait>> = HashMap::new();
        routes.insert("TestEvent".to_string(), Box::new(exact_processor.clone()));
        routes.insert(
            "Test".to_string(),
            Box::new(prefix_processor.clone()) as Box<dyn ProcessorTrait>,
        );

        let router = ProcessorBasedEventRouter { routes };

        let payload = b"test payload";
        let result = router.process_bytes("TestEvent", payload).await;
        assert!(result.is_ok());

        // Exact match should be called
        assert_eq!(exact_processor.calls.lock().unwrap().len(), 1);
        // Prefix match should not be called
        assert_eq!(prefix_processor.calls.lock().unwrap().len(), 0);
    }
}
