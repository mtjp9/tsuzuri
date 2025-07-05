mod common;

use common::{fixtures::*, LocalStackSetup};
use futures::StreamExt;
use tsuzuri::{
    domain_event::SerializedDomainEvent,
    event::SequenceSelect,
    event_store::{AggregateEventStreamer, Persister, SnapshotGetter, SnapshotIntervalProvider},
    integration_event::SerializedIntegrationEvent,
    snapshot::PersistedSnapshot,
    AggregateRoot,
};
use uuid::Uuid;

#[tokio::test]
async fn test_persist_and_stream_domain_events() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "test-01J1234567890ABCDEFGHJKMNP";
    let aggregate_type = TestAggregate::TYPE;

    // Create test events
    let event1 = TestAggregateCreated {
        id: aggregate_id.parse().expect("Failed to parse aggregate_id"),
        name: "Test Aggregate".to_string(),
    };

    let event2 = TestAggregateUpdated { value: 42 };

    let domain_events = vec![
        SerializedDomainEvent {
            id: Uuid::new_v4().to_string(),
            aggregate_id: aggregate_id.to_string(),
            aggregate_type: aggregate_type.to_string(),
            seq_nr: 1,
            event_type: "TestAggregateCreated".to_string(),
            payload: serde_json::to_vec(&event1).unwrap(),
            metadata: Default::default(),
        },
        SerializedDomainEvent {
            id: Uuid::new_v4().to_string(),
            aggregate_id: aggregate_id.to_string(),
            aggregate_type: aggregate_type.to_string(),
            seq_nr: 2,
            event_type: "TestAggregateUpdated".to_string(),
            payload: serde_json::to_vec(&event2).unwrap(),
            metadata: Default::default(),
        },
    ];

    // Persist events
    store
        .persist(&domain_events, &[], None)
        .await
        .expect("Failed to persist events");

    // Stream all events
    let mut stream = store.stream_events::<TestAggregate>(aggregate_id, SequenceSelect::All);
    let mut streamed_events = Vec::new();

    while let Some(event_result) = stream.next().await {
        let event = event_result.expect("Failed to stream event");
        streamed_events.push(event);
    }

    assert_eq!(streamed_events.len(), 2);
    assert_eq!(streamed_events[0].seq_nr, 1);
    assert_eq!(streamed_events[1].seq_nr, 2);

    // Stream from sequence number 2
    let mut stream = store.stream_events::<TestAggregate>(aggregate_id, SequenceSelect::From(2));
    let mut streamed_from_2 = Vec::new();

    while let Some(event_result) = stream.next().await {
        let event = event_result.expect("Failed to stream event");
        streamed_from_2.push(event);
    }

    assert_eq!(streamed_from_2.len(), 1);
    assert_eq!(streamed_from_2[0].seq_nr, 2);
}

#[tokio::test]
async fn test_persist_with_integration_events() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "test-01J1234567890ABCDEFGHJKMNQ";
    let aggregate_type = TestAggregate::TYPE;

    let domain_event = SerializedDomainEvent {
        id: Uuid::new_v4().to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: aggregate_type.to_string(),
        seq_nr: 1,
        event_type: "TestAggregateCreated".to_string(),
        payload: vec![],
        metadata: Default::default(),
    };

    let integration_event = TestIntegrationEvent {
        aggregate_id: aggregate_id.parse().expect("Failed to parse aggregate_id"),
        message: "Integration event message".to_string(),
    };

    let serialized_integration = SerializedIntegrationEvent {
        id: Uuid::new_v4().to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: aggregate_type.to_string(),
        event_type: "TestIntegrationEvent".to_string(),
        payload: serde_json::to_vec(&integration_event).unwrap(),
    };

    // Persist both domain and integration events
    store
        .persist(&[domain_event], &[serialized_integration], None)
        .await
        .expect("Failed to persist events");

    // Verify domain event was persisted
    let mut stream = store.stream_events::<TestAggregate>(aggregate_id, SequenceSelect::All);
    let mut count = 0;

    while let Some(event_result) = stream.next().await {
        event_result.expect("Failed to stream event");
        count += 1;
    }

    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_snapshot_create_and_retrieve() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "test-01J1234567890ABCDEFGHJKMNR";
    let aggregate = TestAggregate {
        id: aggregate_id.parse().expect("Failed to parse aggregate_id"),
        name: "Snapshot Test".to_string(),
        value: 100,
    };

    let snapshot = PersistedSnapshot {
        aggregate_type: TestAggregate::TYPE.to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate: serde_json::to_vec(&aggregate).unwrap(),
        seq_nr: 5,
        version: 1,
    };

    // Create a domain event to persist with snapshot
    let domain_event = SerializedDomainEvent {
        id: Uuid::new_v4().to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: TestAggregate::TYPE.to_string(),
        seq_nr: 5,
        event_type: "TestAggregateUpdated".to_string(),
        payload: vec![],
        metadata: Default::default(),
    };

    // Persist event with snapshot
    store
        .persist(&[domain_event], &[], Some(&snapshot))
        .await
        .expect("Failed to persist with snapshot");

    // Retrieve snapshot
    let retrieved = store
        .get_snapshot::<TestAggregate>(aggregate_id)
        .await
        .expect("Failed to retrieve snapshot");

    assert!(retrieved.is_some());
    let retrieved_snapshot = retrieved.unwrap();
    assert_eq!(retrieved_snapshot.aggregate_id, aggregate_id);
    assert_eq!(retrieved_snapshot.seq_nr, 5);
    assert_eq!(retrieved_snapshot.version, 1);

    // Deserialize and verify aggregate
    let deserialized: TestAggregate = serde_json::from_slice(&retrieved_snapshot.aggregate).unwrap();
    assert_eq!(deserialized.name, "Snapshot Test");
    assert_eq!(deserialized.value, 100);
}

#[tokio::test]
async fn test_snapshot_interval_provider() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    assert_eq!(SnapshotIntervalProvider::snapshot_interval(&store), 10);
}

#[tokio::test]
async fn test_concurrent_event_persistence() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "test-concurrent";
    let aggregate_type = TestAggregate::TYPE;

    // Create first event
    let event1 = SerializedDomainEvent {
        id: Uuid::new_v4().to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: aggregate_type.to_string(),
        seq_nr: 1,
        event_type: "TestAggregateCreated".to_string(),
        payload: vec![],
        metadata: Default::default(),
    };

    // Persist first event
    store
        .persist(&[event1.clone()], &[], None)
        .await
        .expect("Failed to persist first event");

    // Try to persist same sequence number again (should fail)
    let result = store.persist(&[event1], &[], None).await;

    assert!(result.is_err(), "Should fail when persisting duplicate sequence number");
}

#[tokio::test]
async fn test_empty_event_stream() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "non-existent";

    // Stream events for non-existent aggregate
    let mut stream = store.stream_events::<TestAggregate>(aggregate_id, SequenceSelect::All);
    let mut count = 0;

    while stream.next().await.is_some() {
        count += 1;
    }

    assert_eq!(count, 0, "Should return empty stream for non-existent aggregate");
}

#[tokio::test]
async fn test_snapshot_update() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "test-01J1234567890ABCDEFGHJKMNS";
    let aggregate = TestAggregate {
        id: aggregate_id.parse().expect("Failed to parse aggregate_id"),
        name: "Initial".to_string(),
        value: 1,
    };

    // Create initial snapshot
    let snapshot1 = PersistedSnapshot {
        aggregate_type: TestAggregate::TYPE.to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate: serde_json::to_vec(&aggregate).unwrap(),
        seq_nr: 10,
        version: 1,
    };

    let event1 = SerializedDomainEvent {
        id: Uuid::new_v4().to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: TestAggregate::TYPE.to_string(),
        seq_nr: 10,
        event_type: "TestAggregateUpdated".to_string(),
        payload: vec![],
        metadata: Default::default(),
    };

    // Persist first snapshot
    store
        .persist(&[event1], &[], Some(&snapshot1))
        .await
        .expect("Failed to persist first snapshot");

    // Update aggregate
    let updated_aggregate = TestAggregate {
        id: aggregate_id.parse().expect("Failed to parse aggregate_id"),
        name: "Updated".to_string(),
        value: 2,
    };

    // Create updated snapshot
    let snapshot2 = PersistedSnapshot {
        aggregate_type: TestAggregate::TYPE.to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate: serde_json::to_vec(&updated_aggregate).unwrap(),
        seq_nr: 20,
        version: 2,
    };

    let event2 = SerializedDomainEvent {
        id: Uuid::new_v4().to_string(),
        aggregate_id: aggregate_id.to_string(),
        aggregate_type: TestAggregate::TYPE.to_string(),
        seq_nr: 20,
        event_type: "TestAggregateUpdated".to_string(),
        payload: vec![],
        metadata: Default::default(),
    };

    // Persist updated snapshot
    store
        .persist(&[event2], &[], Some(&snapshot2))
        .await
        .expect("Failed to persist updated snapshot");

    // Retrieve latest snapshot
    let retrieved = store
        .get_snapshot::<TestAggregate>(aggregate_id)
        .await
        .expect("Failed to retrieve snapshot")
        .expect("Snapshot should exist");

    assert_eq!(retrieved.version, 2);
    assert_eq!(retrieved.seq_nr, 20);

    let deserialized: TestAggregate = serde_json::from_slice(&retrieved.aggregate).unwrap();
    assert_eq!(deserialized.name, "Updated");
    assert_eq!(deserialized.value, 2);
}
