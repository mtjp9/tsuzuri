use crate::{
    aggregate::AggregateRoot,
    domain_event::SerializedDomainEvent,
    event::{SequenceSelect, Stream},
    integration_event::SerializedIntegrationEvent,
    persist::PersistenceError,
    snapshot::PersistedSnapshot,
};
use async_trait::async_trait;

pub type SnapshotInterval = usize;

/// Trait that defines the capabilities of an event store.
pub trait EventStore:
    SnapshotIntervalProvider + AggregateEventStreamer + Persister + SnapshotGetter + Send + Sync + 'static
{
    /// Calculates the next snapshot interval based on the current sequence number and the number of events.
    /// This method determines when the next snapshot should be taken based on the current sequence number
    /// and the number of events that have occurred since the last snapshot.
    fn commit_snapshot_with_addl_events(&self, current_sequence: usize, num_events: usize) -> usize {
        let max_size = self.snapshot_interval();
        let next_snapshot_at = max_size - (current_sequence % max_size);

        if num_events < next_snapshot_at {
            return 0;
        }

        let addl_events_after_next_snapshot = num_events - next_snapshot_at;
        let addl_events_after_next_snapshot_to_apply =
            addl_events_after_next_snapshot - (addl_events_after_next_snapshot % max_size);
        next_snapshot_at + addl_events_after_next_snapshot_to_apply
    }
}

/// A marker trait for types that can be used as an event store.
impl<T> EventStore for T where
    T: SnapshotIntervalProvider + AggregateEventStreamer + Persister + SnapshotGetter + Send + Sync + 'static
{
}

/// Trait for providing the snapshot interval for the event store.
pub trait SnapshotIntervalProvider: Send + Sync + 'static {
    /// Returns the snapshot interval for the event store.
    ///
    /// The snapshot interval determines how often snapshots are taken to optimize
    /// performance and storage.
    ///
    /// # Returns
    ///
    /// A `SnapshotInterval` value representing the number of events after which a snapshot should be taken.
    fn snapshot_interval(&self) -> SnapshotInterval;
}

/// Trait for streaming aggregate events from the event store.
pub trait AggregateEventStreamer: Send + Sync + 'static {
    fn stream_events<T: AggregateRoot>(
        &self,
        id: &str,
        select: SequenceSelect,
    ) -> Stream<'_, SerializedDomainEvent, PersistenceError>;
}

/// Trait for persisting events and snapshots in the event store.
#[async_trait]
pub trait Persister: Send + Sync + 'static {
    async fn persist(
        &self,
        domain_events: &[SerializedDomainEvent],
        integration_events: &[SerializedIntegrationEvent],
        snapshot_update: Option<&PersistedSnapshot>,
    ) -> Result<(), PersistenceError>;
}

/// Trait for retrieving snapshots from the event store.
#[async_trait]
pub trait SnapshotGetter: Send + Sync + 'static {
    /// Retrieves a snapshot for the given aggregate type and ID.
    async fn get_snapshot<T>(&self, id: &str) -> Result<Option<PersistedSnapshot>, PersistenceError>
    where
        T: AggregateRoot;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_id::{AggregateId, HasIdPrefix},
        command::Command,
        domain_event::DomainEvent,
        event_id::EventIdType,
        integration_event::{self, IntegrationEvent},
        message,
    };
    use futures::stream::{self, StreamExt};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // Test ID type
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct TestId;

    impl HasIdPrefix for TestId {
        const PREFIX: &'static str = "test";
    }

    // Test command
    #[derive(Debug, Clone)]
    struct TestCommand {
        id: AggregateId<TestId>,
    }

    impl message::Message for TestCommand {
        fn name(&self) -> &'static str {
            "TestCommand"
        }
    }

    impl Command for TestCommand {
        type ID = TestId;

        fn id(&self) -> AggregateId<Self::ID> {
            self.id.clone()
        }
    }

    // Test event
    #[derive(Debug, Clone)]
    struct TestEvent {
        id: EventIdType,
    }

    impl message::Message for TestEvent {
        fn name(&self) -> &'static str {
            "TestEvent"
        }
    }

    impl DomainEvent for TestEvent {
        fn id(&self) -> EventIdType {
            self.id
        }

        fn event_type(&self) -> &'static str {
            "TestEvent"
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
    #[allow(dead_code)]
    enum TestError {
        #[error("Test error")]
        TestError,
    }

    // Test aggregate
    #[derive(Debug)]
    struct TestAggregate {
        id: AggregateId<TestId>,
    }

    impl AggregateRoot for TestAggregate {
        const TYPE: &'static str = "TestAggregate";
        type ID = TestId;
        type Command = TestCommand;
        type DomainEvent = TestEvent;
        type IntegrationEvent = TestIntegrationEvent;
        type Error = TestError;

        fn init(id: AggregateId<Self::ID>) -> Self {
            Self { id }
        }

        fn id(&self) -> &AggregateId<Self::ID> {
            &self.id
        }

        fn handle(&mut self, _cmd: Self::Command) -> Result<Self::DomainEvent, Self::Error> {
            Ok(TestEvent { id: EventIdType::new() })
        }

        fn apply(&mut self, _event: Self::DomainEvent) {}
    }

    // Mock event store implementation
    #[derive(Clone)]
    struct MockEventStore {
        snapshot_interval: SnapshotInterval,
        events: Arc<Mutex<HashMap<String, Vec<SerializedDomainEvent>>>>,
        snapshots: Arc<Mutex<HashMap<String, PersistedSnapshot>>>,
        integration_events: Arc<Mutex<Vec<SerializedIntegrationEvent>>>,
    }

    impl MockEventStore {
        fn new(snapshot_interval: SnapshotInterval) -> Self {
            Self {
                snapshot_interval,
                events: Arc::new(Mutex::new(HashMap::new())),
                snapshots: Arc::new(Mutex::new(HashMap::new())),
                integration_events: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl SnapshotIntervalProvider for MockEventStore {
        fn snapshot_interval(&self) -> SnapshotInterval {
            self.snapshot_interval
        }
    }

    impl AggregateEventStreamer for MockEventStore {
        fn stream_events<T: AggregateRoot>(
            &self,
            id: &str,
            select: SequenceSelect,
        ) -> Stream<'_, SerializedDomainEvent, PersistenceError> {
            let events = self.events.lock().unwrap();
            let aggregate_events = events.get(id).cloned().unwrap_or_default();

            let filtered_events: Vec<SerializedDomainEvent> = match select {
                SequenceSelect::All => aggregate_events,
                SequenceSelect::From(seq) => aggregate_events.into_iter().filter(|e| e.seq_nr >= seq).collect(),
            };

            Box::pin(stream::iter(filtered_events.into_iter().map(Ok)))
        }
    }

    #[async_trait]
    impl Persister for MockEventStore {
        async fn persist(
            &self,
            domain_events: &[SerializedDomainEvent],
            integration_events: &[SerializedIntegrationEvent],
            snapshot_update: Option<&PersistedSnapshot>,
        ) -> Result<(), PersistenceError> {
            // Store domain events
            if !domain_events.is_empty() {
                let mut events = self.events.lock().unwrap();
                let aggregate_id = &domain_events[0].aggregate_id;
                events
                    .entry(aggregate_id.clone())
                    .or_default()
                    .extend(domain_events.iter().cloned());
            }

            // Store integration events
            if !integration_events.is_empty() {
                let mut int_events = self.integration_events.lock().unwrap();
                int_events.extend(integration_events.iter().cloned());
            }

            // Update snapshot if provided
            if let Some(snapshot) = snapshot_update {
                let mut snapshots = self.snapshots.lock().unwrap();
                snapshots.insert(
                    snapshot.aggregate_id.clone(),
                    PersistedSnapshot {
                        aggregate_type: snapshot.aggregate_type.clone(),
                        aggregate_id: snapshot.aggregate_id.clone(),
                        aggregate: snapshot.aggregate.clone(),
                        seq_nr: snapshot.seq_nr,
                        version: snapshot.version,
                    },
                );
            }

            Ok(())
        }
    }

    #[async_trait]
    impl SnapshotGetter for MockEventStore {
        async fn get_snapshot<T>(&self, id: &str) -> Result<Option<PersistedSnapshot>, PersistenceError>
        where
            T: AggregateRoot,
        {
            let snapshots = self.snapshots.lock().unwrap();
            Ok(snapshots.get(id).map(|s| PersistedSnapshot {
                aggregate_type: s.aggregate_type.clone(),
                aggregate_id: s.aggregate_id.clone(),
                aggregate: s.aggregate.clone(),
                seq_nr: s.seq_nr,
                version: s.version,
            }))
        }
    }

    #[test]
    fn test_commit_snapshot_with_addl_events() {
        let store = MockEventStore::new(10);

        // Test case 1: No snapshot needed
        assert_eq!(store.commit_snapshot_with_addl_events(5, 3), 0);

        // Test case 2: Exactly at snapshot boundary
        assert_eq!(store.commit_snapshot_with_addl_events(5, 5), 5);

        // Test case 3: Multiple snapshots needed
        assert_eq!(store.commit_snapshot_with_addl_events(5, 25), 25);

        // Test case 4: Just over snapshot boundary
        assert_eq!(store.commit_snapshot_with_addl_events(8, 7), 2);

        // Test case 5: At exact multiple
        assert_eq!(store.commit_snapshot_with_addl_events(10, 10), 10);
    }

    #[test]
    fn test_snapshot_interval_provider() {
        let store = MockEventStore::new(100);
        assert_eq!(store.snapshot_interval(), 100);

        let store2 = MockEventStore::new(50);
        assert_eq!(store2.snapshot_interval(), 50);
    }

    #[test]
    fn test_streamer_trait() {
        futures::executor::block_on(async {
            let store = MockEventStore::new(10);

            // Add some test events
            let events = vec![
                SerializedDomainEvent::new(
                    "evt-1".to_string(),
                    "test-agg-1".to_string(),
                    1,
                    "TestAggregate".to_string(),
                    "TestEvent".to_string(),
                    vec![],
                    json!({}),
                ),
                SerializedDomainEvent::new(
                    "evt-2".to_string(),
                    "test-agg-1".to_string(),
                    2,
                    "TestAggregate".to_string(),
                    "TestEvent".to_string(),
                    vec![],
                    json!({}),
                ),
                SerializedDomainEvent::new(
                    "evt-3".to_string(),
                    "test-agg-1".to_string(),
                    3,
                    "TestAggregate".to_string(),
                    "TestEvent".to_string(),
                    vec![],
                    json!({}),
                ),
            ];

            store.persist(&events, &[], None).await.unwrap();

            // Test streaming all events
            let mut stream = store.stream_events::<TestAggregate>("test-agg-1", SequenceSelect::All);
            let mut count = 0;
            while let Some(result) = stream.next().await {
                let event = result.unwrap();
                count += 1;
                assert_eq!(event.seq_nr, count);
            }
            assert_eq!(count, 3);

            // Test streaming from sequence 2
            let mut stream = store.stream_events::<TestAggregate>("test-agg-1", SequenceSelect::From(2));
            let mut collected = Vec::new();
            while let Some(result) = stream.next().await {
                let event = result.unwrap();
                collected.push(event.seq_nr);
            }
            assert_eq!(collected, vec![2, 3]);
        });
    }

    #[test]
    fn test_snapshot_persister() {
        futures::executor::block_on(async {
            let store = MockEventStore::new(10);

            // Test persisting domain events
            let domain_events = vec![SerializedDomainEvent::new(
                "evt-1".to_string(),
                "test-agg-1".to_string(),
                1,
                "TestAggregate".to_string(),
                "TestEvent".to_string(),
                vec![],
                json!({}),
            )];

            // Test persisting integration events
            let integration_events = vec![SerializedIntegrationEvent::new(
                "int-evt-1".to_string(),
                "test-agg-1".to_string(),
                "TestAggregate".to_string(),
                "test.integration.event".to_string(),
                vec![],
            )];

            // Test persisting with snapshot
            let snapshot = PersistedSnapshot {
                aggregate_type: "TestAggregate".to_string(),
                aggregate_id: "test-agg-1".to_string(),
                aggregate: vec![1, 2, 3],
                seq_nr: 1,
                version: 1,
            };

            let result = store
                .persist(&domain_events, &integration_events, Some(&snapshot))
                .await;

            assert!(result.is_ok());

            // Verify events were stored
            let events = store.events.lock().unwrap();
            assert_eq!(events.get("test-agg-1").unwrap().len(), 1);

            // Verify integration events were stored
            let int_events = store.integration_events.lock().unwrap();
            assert_eq!(int_events.len(), 1);

            // Verify snapshot was stored
            let snapshots = store.snapshots.lock().unwrap();
            assert!(snapshots.contains_key("test-agg-1"));
        });
    }

    #[test]
    fn test_snapshot_getter() {
        futures::executor::block_on(async {
            let store = MockEventStore::new(10);

            // Test getting non-existent snapshot
            let result = store.get_snapshot::<TestAggregate>("non-existent").await;
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());

            // Add a snapshot
            let snapshot = PersistedSnapshot {
                aggregate_type: "TestAggregate".to_string(),
                aggregate_id: "test-agg-1".to_string(),
                aggregate: vec![10, 20, 30],
                seq_nr: 50,
                version: 5,
            };

            store.persist(&[], &[], Some(&snapshot)).await.unwrap();

            // Test getting existing snapshot
            let result = store.get_snapshot::<TestAggregate>("test-agg-1").await;
            assert!(result.is_ok());
            let retrieved = result.unwrap().unwrap();
            assert_eq!(retrieved.aggregate_id, "test-agg-1");
            assert_eq!(retrieved.version, 5);
            assert_eq!(retrieved.seq_nr, 50);
            assert_eq!(retrieved.aggregate, vec![10, 20, 30]);
        });
    }

    #[test]
    fn test_integration_flow() {
        futures::executor::block_on(async {
            let store = MockEventStore::new(5);

            // Create events for multiple snapshots
            let mut all_events = Vec::new();
            for i in 1..=12 {
                all_events.push(SerializedDomainEvent::new(
                    format!("evt-{i}"),
                    "test-agg-1".to_string(),
                    i,
                    "TestAggregate".to_string(),
                    "TestEvent".to_string(),
                    vec![],
                    json!({"index": i}),
                ));
            }

            // Persist events in batches
            store.persist(&all_events[0..5], &[], None).await.unwrap();

            // Check if snapshot is needed
            let snapshot_at = store.commit_snapshot_with_addl_events(0, 5);
            assert_eq!(snapshot_at, 5);

            // Create and persist snapshot
            let snapshot = PersistedSnapshot {
                aggregate_type: "TestAggregate".to_string(),
                aggregate_id: "test-agg-1".to_string(),
                aggregate: vec![1, 2, 3, 4, 5],
                seq_nr: 5,
                version: 1,
            };

            store.persist(&all_events[5..10], &[], Some(&snapshot)).await.unwrap();

            // Verify we can stream all events
            let mut stream = store.stream_events::<TestAggregate>("test-agg-1", SequenceSelect::All);
            let mut count = 0;
            while stream.next().await.is_some() {
                count += 1;
            }
            assert_eq!(count, 10);

            // Verify snapshot exists
            let retrieved_snapshot = store.get_snapshot::<TestAggregate>("test-agg-1").await.unwrap();
            assert!(retrieved_snapshot.is_some());
            assert_eq!(retrieved_snapshot.unwrap().seq_nr, 5);
        });
    }
}
