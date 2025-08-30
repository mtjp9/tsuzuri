use crate::{domain_event::DomainEvent, integration_event::IntegrationEvent};
use std::fmt::Debug;

/// Adapter trait for converting domain events to integration events at the application layer.
/// This trait should be implemented by application services to handle the conversion
/// of domain events to integration events based on business requirements.
pub trait IntegrationEventAdapter: Debug + Send + Sync + 'static {
    type DomainEvent: DomainEvent;
    type IntegrationEvent: IntegrationEvent;

    /// Converts a domain event into zero or more integration events.
    /// Returns None if no integration event should be published for this domain event.
    fn to_integration_events(&self, domain_event: Self::DomainEvent) -> Vec<Self::IntegrationEvent>;
}

/// No-op adapter that produces no integration events.
/// Useful for aggregates that don't need to publish integration events.
#[derive(Debug, Clone)]
pub struct NoOpIntegrationAdapter<D, I> {
    _phantom: std::marker::PhantomData<(D, I)>,
}

impl<D, I> NoOpIntegrationAdapter<D, I> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<D, I> Default for NoOpIntegrationAdapter<D, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D, I> IntegrationEventAdapter for NoOpIntegrationAdapter<D, I>
where
    D: DomainEvent,
    I: IntegrationEvent,
{
    type DomainEvent = D;
    type IntegrationEvent = I;

    fn to_integration_events(&self, _domain_event: Self::DomainEvent) -> Vec<Self::IntegrationEvent> {
        vec![]
    }
}
