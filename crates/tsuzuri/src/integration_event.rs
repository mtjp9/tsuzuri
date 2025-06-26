/// Marker trait for integration events that communicate changes to external systems.
/// Integration events are used for communication between bounded contexts.
pub trait IntegrationEvent: Send + Sync + 'static {}
