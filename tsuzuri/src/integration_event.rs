use crate::{domain_event::DomainEvent, message};
use std::fmt;

/// Marker trait for integration events that communicate changes to external systems.
/// Integration events are used for communication between bounded contexts.
pub trait IntegrationEvent: fmt::Debug + message::Message + Send + Sync + 'static {
    fn id(&self) -> String;
    fn event_type(&self) -> &'static str;
}

pub trait IntoIntegrationEvents: DomainEvent {
    type IntegrationEvent: IntegrationEvent;
    type IntoIter: IntoIterator<Item = Self::IntegrationEvent>;

    fn into_integration_events(self) -> Self::IntoIter;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerializedIntegrationEvent {
    pub id: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub event_type: String,
    pub payload: Vec<u8>,
}

#[allow(dead_code)]
impl SerializedIntegrationEvent {
    pub fn new(id: String, aggregate_id: String, aggregate_type: String, event_type: String, payload: Vec<u8>) -> Self {
        Self {
            id,
            aggregate_id,
            aggregate_type,
            event_type,
            payload,
        }
    }
}
