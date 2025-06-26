/// Marker trait for commands that can be handled by aggregates.
/// Commands represent intentions to change the state of an aggregate.
pub trait Command: Send + Sync + 'static {}
