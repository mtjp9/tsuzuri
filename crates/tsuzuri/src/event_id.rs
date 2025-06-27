use crate::aggregate_id::{AggregateId, HasIdPrefix};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId;

impl HasIdPrefix for EventId {
    const PREFIX: &'static str = "evt";
}

pub type EventIdType = AggregateId<EventId>;
