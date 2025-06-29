use crate::{aggregate::AggregateRoot, aggregate_id::AggregateId, sequence_number::SequenceNumber, version::Version};

/// A wrapper around an aggregate root that tracks version and sequence number
/// for event sourcing and optimistic concurrency control.
#[derive(Debug, PartialEq)]
#[must_use]
pub struct VersionedAggregate<T: AggregateRoot> {
    aggregate: T,
    version: Version,
    seq_nr: SequenceNumber,
}

impl<T: AggregateRoot> VersionedAggregate<T> {
    /// Creates a new VersionedAggregate with the given aggregate, version, and sequence number.
    pub fn new(aggregate: T, version: Version, seq_nr: SequenceNumber) -> Self {
        Self {
            aggregate,
            version,
            seq_nr,
        }
    }

    /// Returns a reference to the aggregate ID.
    pub fn id(&self) -> &AggregateId<T::ID> {
        self.aggregate.id()
    }

    /// Returns a reference to the aggregate.
    pub fn aggregate(&self) -> &T {
        &self.aggregate
    }

    /// Returns the current version.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Returns the current sequence number.
    pub fn seq_nr(&self) -> SequenceNumber {
        self.seq_nr
    }

    pub fn set_seq_nr(&mut self, seq_nr: SequenceNumber) {
        self.seq_nr = seq_nr;
    }

    pub fn handle(&mut self, cmd: T::Command) -> Result<T::DomainEvent, T::Error> {
        let event = self.aggregate.handle(cmd)?;
        Ok(event)
    }

    pub fn apply(&mut self, event: T::DomainEvent) {
        self.aggregate.apply(event);
    }

    pub fn snapshot(&self) -> (&T, Version, SequenceNumber) {
        (self.aggregate(), self.version, self.seq_nr)
    }

    pub fn from_snapshot(aggregate: T, version: Version, seq_nr: SequenceNumber) -> Self {
        Self::new(aggregate, version, seq_nr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_id::HasIdPrefix,
        command::Command,
        domain_event::DomainEvent,
        event_id::EventIdType,
        integration_event::{self, IntegrationEvent},
        message,
    };

    // Test ID types
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct TestId;

    impl HasIdPrefix for TestId {
        const PREFIX: &'static str = "test";
    }

    // Test command
    #[derive(Debug, Clone)]
    enum TestCommand {
        DoSomething { id: AggregateId<TestId> },
        DoSomethingElse { id: AggregateId<TestId> },
        CausesError { id: AggregateId<TestId> },
    }

    impl message::Message for TestCommand {
        fn name(&self) -> &'static str {
            "TestCommand"
        }
    }

    impl Command for TestCommand {
        type ID = TestId;

        fn id(&self) -> AggregateId<Self::ID> {
            match self {
                Self::DoSomething { id } => id.clone(),
                Self::DoSomethingElse { id } => id.clone(),
                Self::CausesError { id } => id.clone(),
            }
        }
    }

    // Test event
    #[derive(Debug, Clone, PartialEq)]
    enum TestEvent {
        SomethingHappened { id: EventIdType, data: String },
        SomethingElseHappened { id: EventIdType, data: String },
    }

    impl message::Message for TestEvent {
        fn name(&self) -> &'static str {
            "TestEvent"
        }
    }

    impl DomainEvent for TestEvent {
        fn id(&self) -> EventIdType {
            match self {
                Self::SomethingHappened { id, .. } => *id,
                Self::SomethingElseHappened { id, .. } => *id,
            }
        }

        fn event_type(&self) -> &'static str {
            match self {
                Self::SomethingHappened { .. } => "SomethingHappened",
                Self::SomethingElseHappened { .. } => "SomethingElseHappened",
            }
        }
    }

    impl integration_event::IntoIntegrationEvents for TestEvent {
        type IntegrationEvent = TestIntegrationEvent;
        type IntoIter = Vec<TestIntegrationEvent>;

        fn into_integration_events(self) -> Self::IntoIter {
            vec![TestIntegrationEvent]
        }
    }

    // Test integration event
    #[derive(Debug, Clone)]
    struct TestIntegrationEvent;

    impl message::Message for TestIntegrationEvent {
        fn name(&self) -> &'static str {
            "TestIntegrationEvent"
        }
    }

    impl IntegrationEvent for TestIntegrationEvent {
        fn id(&self) -> String {
            ulid::Ulid::new().to_string()
        }

        fn event_type(&self) -> &'static str {
            "test.integration.event"
        }
    }

    // Test error
    #[derive(Debug, thiserror::Error)]
    enum TestError {
        #[error("Something went wrong")]
        SomethingWentWrong,
    }

    // Test aggregate
    #[derive(Debug, PartialEq)]
    struct TestAggregate {
        id: AggregateId<TestId>,
        state: String,
    }

    impl AggregateRoot for TestAggregate {
        const TYPE: &'static str = "TestAggregate";
        type ID = TestId;
        type Command = TestCommand;
        type DomainEvent = TestEvent;
        type IntegrationEvent = TestIntegrationEvent;
        type Error = TestError;

        fn init(id: AggregateId<Self::ID>) -> Self {
            Self {
                id,
                state: "initial".to_string(),
            }
        }

        fn id(&self) -> &AggregateId<Self::ID> {
            &self.id
        }

        fn handle(&mut self, cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
            match cmd {
                TestCommand::DoSomething { .. } => Ok(TestEvent::SomethingHappened {
                    id: EventIdType::new(),
                    data: "something".to_string(),
                }),
                TestCommand::DoSomethingElse { .. } => Ok(TestEvent::SomethingElseHappened {
                    id: EventIdType::new(),
                    data: "something else".to_string(),
                }),
                TestCommand::CausesError { .. } => Err(TestError::SomethingWentWrong),
            }
        }

        fn apply(&mut self, event: Self::DomainEvent) {
            match event {
                TestEvent::SomethingHappened { data, .. } => {
                    self.state = format!("{} -> {}", self.state, data);
                }
                TestEvent::SomethingElseHappened { data, .. } => {
                    self.state = format!("{} -> {}", self.state, data);
                }
            }
        }
    }

    // Helper function to create a test VersionedAggregate
    fn create_test_versioned_aggregate() -> VersionedAggregate<TestAggregate> {
        let aggregate = TestAggregate {
            id: AggregateId::<TestId>::new(),
            state: "initial".to_string(),
        };

        VersionedAggregate::new(aggregate, 1, 0)
    }

    #[test]
    fn test_versioned_aggregate_creation() {
        let versioned = create_test_versioned_aggregate();

        assert_eq!(versioned.aggregate.state, "initial");
        assert_eq!(versioned.version, 1);
        assert_eq!(versioned.seq_nr, 0);
    }

    #[test]
    fn test_handle_command() {
        let mut versioned = create_test_versioned_aggregate();
        let cmd = TestCommand::DoSomething { id: *versioned.id() };

        let result = versioned.handle(cmd);
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(matches!(event, TestEvent::SomethingHappened { .. }));

        // Aggregate state should NOT be updated yet (apply not called in handle)
        assert_eq!(versioned.aggregate.state, "initial");
    }

    #[test]
    fn test_apply_event() {
        let mut versioned = create_test_versioned_aggregate();
        let event = TestEvent::SomethingHappened {
            id: EventIdType::new(),
            data: "test data".to_string(),
        };

        versioned.apply(event);

        // State should be updated
        assert_eq!(versioned.aggregate.state, "initial -> test data");
    }

    #[test]
    fn test_handle_multiple_commands() {
        let mut versioned = create_test_versioned_aggregate();

        // Handle multiple commands
        let cmd1 = TestCommand::DoSomething { id: *versioned.id() };
        let cmd2 = TestCommand::DoSomethingElse { id: *versioned.id() };

        let event1 = versioned.handle(cmd1).unwrap();
        let event2 = versioned.handle(cmd2).unwrap();

        assert!(matches!(event1, TestEvent::SomethingHappened { .. }));
        assert!(matches!(event2, TestEvent::SomethingElseHappened { .. }));

        // Aggregate state should still be initial (events not applied)
        assert_eq!(versioned.aggregate.state, "initial");
    }

    #[test]
    fn test_snapshot() {
        let versioned = create_test_versioned_aggregate();
        let (aggregate, version, seq_nr) = versioned.snapshot();

        assert_eq!(aggregate.state, "initial");
        assert_eq!(version, 1);
        assert_eq!(seq_nr, 0);
    }

    #[test]
    fn test_handle_command_error() {
        let mut versioned = create_test_versioned_aggregate();
        let cmd = TestCommand::CausesError { id: *versioned.id() };

        let result = versioned.handle(cmd);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TestError::SomethingWentWrong));
    }

    #[test]
    fn test_multiple_commands_sequence() {
        let mut versioned = create_test_versioned_aggregate();
        let mut events = Vec::new();

        // Handle multiple commands successfully
        for i in 0..3 {
            let cmd = if i % 2 == 0 {
                TestCommand::DoSomething { id: *versioned.id() }
            } else {
                TestCommand::DoSomethingElse { id: *versioned.id() }
            };

            let event = versioned.handle(cmd).unwrap();
            events.push(event);
        }

        assert_eq!(events.len(), 3);

        // Apply events to verify they update the state correctly
        for event in events {
            versioned.apply(event);
        }

        // State should be updated through all events
        assert_eq!(
            versioned.aggregate.state,
            "initial -> something -> something else -> something"
        );
    }
}
