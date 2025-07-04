use async_trait::async_trait;
use std::collections::HashMap;
use tsuzuri::{
    domain_event::DomainEvent,
    event::Envelope,
    projection::{
        adapter::{Adapter, Projector},
        error::Result,
        processor::Processor,
    },
    serde::Serde,
};

/// Alternative: Type-safe event router with deserialization
pub struct TypedEventRouter<E> {
    routes: HashMap<String, Box<dyn Projector<E>>>,
    _phantom: std::marker::PhantomData<E>,
}

impl<E> TypedEventRouter<E>
where
    E: DomainEvent,
{
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn routes(&self) -> &HashMap<String, Box<dyn Projector<E>>> {
        &self.routes
    }

    pub fn route(mut self, event_name: &str, projector: Box<dyn Projector<E>>) -> Self {
        self.routes.insert(event_name.to_string(), projector);
        self
    }
}

impl<E> Default for TypedEventRouter<E>
where
    E: DomainEvent,
{
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<E> Projector<E> for TypedEventRouter<E>
where
    E: DomainEvent + Send + Sync,
{
    async fn project(&self, event: Envelope<E>) -> Result<()> {
        let event_name = event.message.name();

        match self.routes.get(event_name) {
            Some(projector) => projector.project(event).await,
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
    async fn process_bytes(&self, payload: &[u8], metadata: &[u8]) -> Result<()>;
}

impl ProcessorBasedEventRouter {
    pub fn new() -> Self {
        Self { routes: HashMap::new() }
    }

    /// Register a processor for an event type prefix
    /// Example: registering "ProjectDomainEvent" will match "ProjectDomainEventBodyChanged"
    pub fn route_processor<A, E, EvtSerde>(mut self, event_prefix: &str, processor: Processor<A, E, EvtSerde>) -> Self
    where
        A: Adapter<E> + 'static,
        E: DomainEvent + 'static,
        EvtSerde: Serde<E> + 'static,
    {
        self.routes
            .insert(event_prefix.to_string(), Box::new(ProcessorWrapper { processor }));
        self
    }

    /// Process bytes through appropriate processor
    /// Each processor will handle its own deserialization using its own Serde implementation
    /// Uses prefix matching: "ProjectDomainEvent" matches "ProjectDomainEventBodyChanged"
    pub async fn process_bytes(&self, event_name: &str, payload: &[u8], metadata: &[u8]) -> Result<()> {
        // First try exact match
        if let Some(processor) = self.routes.get(event_name) {
            return processor.process_bytes(payload, metadata).await;
        }

        // Then try prefix match
        for (registered_prefix, processor) in &self.routes {
            if event_name.starts_with(registered_prefix) {
                return processor.process_bytes(payload, metadata).await;
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
    E: DomainEvent + Send + Sync,
    EvtSerde: Serde<E> + Send + Sync,
{
    async fn process_bytes(&self, payload: &[u8], metadata: &[u8]) -> Result<()> {
        self.processor.process_bytes(payload, metadata).await
    }
}
