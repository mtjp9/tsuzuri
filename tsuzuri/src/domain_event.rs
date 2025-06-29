use crate::{event_id::EventIdType, message, sequence_number::SequenceNumber};
use serde_json::Value;
use std::fmt;

/// Marker trait for domain events that represent state changes within an aggregate.
/// Domain events capture what happened in the domain.
pub trait DomainEvent: fmt::Debug + Clone + message::Message + Send + Sync + 'static {
    fn id(&self) -> EventIdType;
    fn event_type(&self) -> &'static str;
    fn index_keywords(&self) -> Vec<String> {
        vec![]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerializedDomainEvent {
    pub id: String,
    pub aggregate_id: String,
    pub seq_nr: SequenceNumber,
    pub aggregate_type: String,
    pub event_type: String,
    pub payload: Vec<u8>,
    pub metadata: Value,
}

#[allow(dead_code)]
impl SerializedDomainEvent {
    pub fn new(
        id: String,
        aggregate_id: String,
        seq_nr: SequenceNumber,
        aggregate_type: String,
        event_type: String,
        payload: Vec<u8>,
        metadata: Value,
    ) -> Self {
        Self {
            id,
            aggregate_id,
            seq_nr,
            aggregate_type,
            event_type,
            payload,
            metadata,
        }
    }
}
