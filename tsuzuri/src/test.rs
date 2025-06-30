//! Test framework for tsuzuri aggregates
//!
//! This module provides a fluent test framework for testing aggregates, commands, and events
//! using a Given-When-Then pattern similar to behavior-driven development (BDD).

use crate::aggregate::AggregateRoot;
use std::fmt::Debug;
use std::marker::PhantomData;

/// Test framework for testing aggregates with a Given-When-Then pattern
pub struct TestFramework<A: AggregateRoot> {
    aggregate: A,
    _phantom: PhantomData<A>,
}

impl<A: AggregateRoot> TestFramework<A> {
    /// Creates a test framework with a custom aggregate instance
    pub fn with(aggregate: A) -> Self {
        Self {
            aggregate,
            _phantom: PhantomData,
        }
    }
}

/// Given phase - setup initial state
impl<A: AggregateRoot> TestFramework<A> {
    /// Start with no previous events (clean state)
    pub fn given_no_previous_events(self) -> WhenPhase<A> {
        WhenPhase {
            aggregate: self.aggregate,
            initial_events: Vec::new(),
        }
    }

    /// Start with a set of previous events
    pub fn given(mut self, events: Vec<A::DomainEvent>) -> WhenPhase<A> {
        // Apply all events to aggregate to build up state
        for event in &events {
            self.aggregate.apply(event.clone());
        }

        WhenPhase {
            aggregate: self.aggregate,
            initial_events: events,
        }
    }

    /// Start with a single previous event
    pub fn given_event(self, event: A::DomainEvent) -> WhenPhase<A> {
        self.given(vec![event])
    }
}

/// When phase - execute command
pub struct WhenPhase<A: AggregateRoot> {
    aggregate: A,
    initial_events: Vec<A::DomainEvent>,
}

impl<A: AggregateRoot> WhenPhase<A> {
    /// Execute a command on the aggregate
    pub fn when(mut self, command: A::Command) -> ThenPhase<A> {
        let result = self.aggregate.handle(command);

        // Convert single event result to Vec for consistent handling
        let vec_result = result.map(|event| vec![event]);

        ThenPhase {
            aggregate: self.aggregate,
            initial_events: self.initial_events,
            result: vec_result,
        }
    }
}

/// Then phase - verify outcomes
pub struct ThenPhase<A: AggregateRoot> {
    aggregate: A,
    #[allow(dead_code)]
    initial_events: Vec<A::DomainEvent>,
    result: Result<Vec<A::DomainEvent>, A::Error>,
}

impl<A: AggregateRoot> ThenPhase<A>
where
    A::DomainEvent: Debug + PartialEq,
    A::Error: Debug,
{
    /// Verify that the expected events were produced
    pub fn then_expect_events(self, expected_events: Vec<A::DomainEvent>) {
        match self.result {
            Ok(actual_events) => {
                assert_eq!(
                    actual_events, expected_events,
                    "Expected events do not match actual events.\nExpected: {expected_events:?}\nActual: {actual_events:?}"
                );
            }
            Err(e) => {
                panic!("Expected events but got error: {e:?}");
            }
        }
    }

    /// Verify that a single event was produced
    pub fn then_expect_event(self, expected_event: A::DomainEvent) {
        self.then_expect_events(vec![expected_event])
    }

    /// Verify that no events were produced
    pub fn then_expect_no_events(self) {
        self.then_expect_events(vec![])
    }

    /// Verify that an error was produced
    pub fn then_expect_error<E>(self) -> E
    where
        E: Debug,
        A::Error: Into<E>,
    {
        match self.result {
            Ok(events) => {
                panic!("Expected error but got events: {events:?}");
            }
            Err(error) => error.into(),
        }
    }

    /// Verify that a specific error matches
    pub fn then_expect_error_matches<F>(self, predicate: F)
    where
        F: FnOnce(&A::Error) -> bool,
    {
        match self.result {
            Ok(events) => {
                panic!("Expected error but got events: {events:?}");
            }
            Err(ref error) => {
                assert!(predicate(error), "Error does not match expected predicate: {error:?}");
            }
        }
    }

    /// Get the final aggregate state after command execution
    pub fn then_aggregate_state<F>(mut self, assertion: F)
    where
        F: FnOnce(&A),
    {
        // Apply resulting events if successful
        if let Ok(events) = &self.result {
            for event in events {
                self.aggregate.apply(event.clone());
            }
        }

        assertion(&self.aggregate);
    }

    /// Get access to the result for custom assertions
    pub fn then_verify<F>(self, verification: F)
    where
        F: FnOnce(Result<Vec<A::DomainEvent>, A::Error>),
    {
        verification(self.result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_id::{AggregateId, HasIdPrefix},
        command::Command,
        domain_event::DomainEvent,
        event_id::EventIdType,
        integration_event::{IntegrationEvent, IntoIntegrationEvents},
        message::Message,
        AggregateRoot,
    };

    // Test ID type
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct TestId;

    impl HasIdPrefix for TestId {
        const PREFIX: &'static str = "test";
    }

    // Test aggregate for verifying the test framework
    #[derive(Debug, Clone, PartialEq)]
    struct TestAggregate {
        id: AggregateId<TestId>,
        value: i32,
        is_active: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestCommand {
        Create { id: AggregateId<TestId> },
        UpdateValue { value: i32 },
        Deactivate,
    }

    impl Message for TestCommand {
        fn name(&self) -> &'static str {
            match self {
                TestCommand::Create { .. } => "Create",
                TestCommand::UpdateValue { .. } => "UpdateValue",
                TestCommand::Deactivate => "Deactivate",
            }
        }
    }

    impl Command for TestCommand {
        type ID = TestId;

        fn id(&self) -> AggregateId<Self::ID> {
            match self {
                TestCommand::Create { id } => *id,
                TestCommand::UpdateValue { .. } => panic!("UpdateValue command requires aggregate to exist"),
                TestCommand::Deactivate => panic!("Deactivate command requires aggregate to exist"),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestEvent {
        Created { id: AggregateId<TestId> },
        ValueUpdated { value: i32 },
        Deactivated,
    }

    impl Message for TestEvent {
        fn name(&self) -> &'static str {
            match self {
                TestEvent::Created { .. } => "Created",
                TestEvent::ValueUpdated { .. } => "ValueUpdated",
                TestEvent::Deactivated => "Deactivated",
            }
        }
    }

    impl DomainEvent for TestEvent {
        fn id(&self) -> EventIdType {
            EventIdType::new()
        }

        fn event_type(&self) -> &'static str {
            self.name()
        }
    }

    // Integration event for test
    #[derive(Debug, Clone)]
    struct TestIntegrationEvent {
        #[allow(dead_code)]
        message: String,
    }

    impl Message for TestIntegrationEvent {
        fn name(&self) -> &'static str {
            "TestIntegrationEvent"
        }
    }

    impl IntegrationEvent for TestIntegrationEvent {
        fn id(&self) -> String {
            ulid::Ulid::new().to_string()
        }

        fn event_type(&self) -> &'static str {
            "TestIntegrationEvent"
        }
    }

    impl IntoIntegrationEvents for TestEvent {
        type IntegrationEvent = TestIntegrationEvent;
        type IntoIter = std::vec::IntoIter<Self::IntegrationEvent>;

        fn into_integration_events(self) -> Self::IntoIter {
            let message = match &self {
                TestEvent::Created { id } => format!("Created with id: {id}"),
                TestEvent::ValueUpdated { value } => format!("Updated value to: {value}"),
                TestEvent::Deactivated => "Deactivated".to_string(),
            };
            vec![TestIntegrationEvent { message }].into_iter()
        }
    }

    #[derive(Debug, thiserror::Error)]
    enum TestError {
        #[error("Already created")]
        AlreadyCreated,
        #[error("Not active")]
        NotActive,
        #[error("Invalid value")]
        InvalidValue,
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
                value: 0,
                is_active: false,
            }
        }

        fn id(&self) -> &AggregateId<Self::ID> {
            &self.id
        }

        fn handle(&mut self, command: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
            match command {
                TestCommand::Create { id } => {
                    if self.is_active {
                        return Err(TestError::AlreadyCreated);
                    }
                    Ok(TestEvent::Created { id })
                }
                TestCommand::UpdateValue { value } => {
                    if !self.is_active {
                        return Err(TestError::NotActive);
                    }
                    if value < 0 {
                        return Err(TestError::InvalidValue);
                    }
                    Ok(TestEvent::ValueUpdated { value })
                }
                TestCommand::Deactivate => {
                    if !self.is_active {
                        return Err(TestError::NotActive);
                    }
                    Ok(TestEvent::Deactivated)
                }
            }
        }

        fn apply(&mut self, event: Self::DomainEvent) {
            match event {
                TestEvent::Created { id } => {
                    self.id = id;
                    self.is_active = true;
                }
                TestEvent::ValueUpdated { value } => {
                    self.value = value;
                }
                TestEvent::Deactivated => {
                    self.is_active = false;
                }
            }
        }
    }

    #[test]
    fn test_given_no_previous_events() {
        let id = AggregateId::<TestId>::new();
        let aggregate = TestAggregate::init(id);

        TestFramework::with(aggregate)
            .given_no_previous_events()
            .when(TestCommand::Create { id })
            .then_expect_event(TestEvent::Created { id });
    }

    #[test]
    fn test_given_with_events() {
        let id = AggregateId::<TestId>::new();
        let aggregate = TestAggregate::init(id);

        TestFramework::with(aggregate)
            .given(vec![TestEvent::Created { id }])
            .when(TestCommand::UpdateValue { value: 42 })
            .then_expect_event(TestEvent::ValueUpdated { value: 42 });
    }

    #[test]
    fn test_expect_error() {
        let id = AggregateId::<TestId>::new();
        let aggregate = TestAggregate::init(id);

        TestFramework::with(aggregate)
            .given_no_previous_events()
            .when(TestCommand::UpdateValue { value: 10 })
            .then_expect_error::<TestError>();
    }

    #[test]
    fn test_expect_error_matches() {
        let id = AggregateId::<TestId>::new();
        let aggregate = TestAggregate::init(id);

        TestFramework::with(aggregate)
            .given_no_previous_events()
            .when(TestCommand::UpdateValue { value: 10 })
            .then_expect_error_matches(|e| matches!(e, TestError::NotActive));
    }

    #[test]
    fn test_aggregate_state_assertion() {
        let id = AggregateId::<TestId>::new();
        let aggregate = TestAggregate::init(id);

        TestFramework::with(aggregate)
            .given(vec![TestEvent::Created { id }])
            .when(TestCommand::UpdateValue { value: 99 })
            .then_aggregate_state(|agg| {
                assert_eq!(*agg.id(), id);
                assert_eq!(agg.value, 99);
                assert!(agg.is_active);
            });
    }

    #[test]
    fn test_deactivate_already_inactive() {
        let id = AggregateId::<TestId>::new();
        let aggregate = TestAggregate::init(id);

        TestFramework::with(aggregate)
            .given(vec![TestEvent::Created { id }, TestEvent::Deactivated])
            .when(TestCommand::Deactivate)
            .then_expect_error_matches(|e| matches!(e, TestError::NotActive));
    }
}
