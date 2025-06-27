use crate::{
    aggregate_id::{AggregateId, HasIdPrefix},
    message,
};
use std::fmt;

pub mod handler;
pub mod repository;

#[allow(dead_code)]
pub type Envelope<T> = message::Envelope<T>;

/// Marker trait for commands that can be handled by aggregates.
/// Commands represent intentions to change the state of an aggregate.
pub trait Command: fmt::Debug + message::Message + Send + Sync + 'static {
    type ID: HasIdPrefix;

    fn id(&self) -> &AggregateId<Self::ID>;
}
