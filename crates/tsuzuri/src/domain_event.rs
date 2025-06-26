/// Marker trait for domain events that represent state changes within an aggregate.
/// Domain events capture what happened in the domain.
pub trait DomainEvent: Send + Sync + 'static {}
