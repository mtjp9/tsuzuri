/// This file defines the types and traits used in the event system of Tsuzuri.
use crate::{message, sequence_number::SequenceNumber};
use futures::stream::BoxStream;
use std::collections::HashMap;

pub type Envelope<T> = message::Envelope<T>;
pub type Metadata = HashMap<String, String>;
pub type Stream<'a, SerializedDomainEvent, Err> = BoxStream<'a, Result<SerializedDomainEvent, Err>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceSelect {
    All,
    From(SequenceNumber),
}
