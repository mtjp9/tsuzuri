mod common;
use common::fixtures::{
    CreateTestAggregate, TestAggregate, TestAggregateCreated, TestAggregateUpdated, TestCommand, TestDomainEvent,
    UpdateTestAggregate,
};
use tsuzuri::aggregate_id::AggregateId;
use tsuzuri::test::TestFramework;
use tsuzuri::AggregateRoot;

#[test]
fn test_create_aggregate_with_test_framework() {
    let id = AggregateId::new();
    let aggregate = TestAggregate::init(id);

    // Test create command
    TestFramework::with(aggregate)
        .given_no_previous_events()
        .when(TestCommand::Create(CreateTestAggregate {
            id,
            name: "Test Aggregate".to_string(),
        }))
        .then_verify(|result| {
            assert!(result.is_ok());
            let events = result.unwrap();
            assert_eq!(events.len(), 1);
            match &events[0] {
                TestDomainEvent::Created(event) => {
                    assert_eq!(event.name, "Test Aggregate");
                }
                _ => panic!("Expected TestDomainEvent::Created"),
            }
        });
}

#[test]
fn test_update_aggregate_with_test_framework() {
    let id = AggregateId::new();
    let aggregate = TestAggregate::init(id);

    // Test update command after creation
    TestFramework::with(aggregate)
        .given(vec![TestDomainEvent::Created(TestAggregateCreated {
            id,
            name: "Test Aggregate".to_string(),
        })])
        .when(TestCommand::Update(UpdateTestAggregate { value: 100 }))
        .then_verify(|result| {
            assert!(result.is_ok());
            let events = result.unwrap();
            assert_eq!(events.len(), 1);
            match &events[0] {
                TestDomainEvent::Updated(event) => {
                    assert_eq!(event.value, 100);
                }
                _ => panic!("Expected TestDomainEvent::Updated"),
            }
        });
}

#[test]
fn test_aggregate_state_after_events() {
    let id = AggregateId::new();
    let aggregate = TestAggregate::init(id);

    // Test aggregate state after applying events
    TestFramework::with(aggregate)
        .given(vec![TestDomainEvent::Created(TestAggregateCreated {
            id,
            name: "Initial Name".to_string(),
        })])
        .when(TestCommand::Update(UpdateTestAggregate { value: 200 }))
        .then_aggregate_state(|agg| {
            assert_eq!(agg.name, "Initial Name");
            assert_eq!(agg.value, 200);
        });
}

#[test]
fn test_multiple_events_sequence() {
    let id = AggregateId::new();
    let aggregate = TestAggregate::init(id);

    // Test multiple events in sequence
    TestFramework::with(aggregate)
        .given(vec![
            TestDomainEvent::Created(TestAggregateCreated {
                id,
                name: "Test".to_string(),
            }),
            TestDomainEvent::Updated(TestAggregateUpdated { value: 10 }),
            TestDomainEvent::Updated(TestAggregateUpdated { value: 100 }),
        ])
        .when(TestCommand::Update(UpdateTestAggregate { value: 1000 }))
        .then_aggregate_state(|agg| {
            // Should have the final value after all updates
            assert_eq!(agg.value, 1000);
            assert_eq!(agg.name, "Test");
        });
}

#[test]
fn test_aggregate_initialization() {
    let id = AggregateId::new();
    let aggregate = TestAggregate::init(id);

    // Test initial state
    TestFramework::with(aggregate)
        .given_no_previous_events()
        .when(TestCommand::Update(UpdateTestAggregate { value: 50 }))
        .then_aggregate_state(|agg| {
            // Name should be empty from init
            assert_eq!(agg.name, "");
            // Value should be updated to 50
            assert_eq!(agg.value, 50);
        });
}
