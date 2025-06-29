use crate::{
    aggregate::AggregateRoot,
    domain_event::SerializedDomainEvent,
    event::{SequenceSelect, Stream},
    event_store::{AggregateEventStreamer, Persister, SnapshotGetter, SnapshotIntervalProvider},
    integration_event::SerializedIntegrationEvent,
    inverted_index_store::{AggregateIdsLoader, InvertedIndexCommiter, InvertedIndexRemover},
    persist::PersistenceError,
    snapshot::PersistedSnapshot,
};
use async_trait::async_trait;
use futures::stream;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Memory-based event store for testing and development
#[derive(Clone)]
pub struct MemoryEventStore {
    snapshot_interval: usize,
    events: Arc<RwLock<HashMap<String, Vec<SerializedDomainEvent>>>>,
    snapshots: Arc<RwLock<HashMap<String, PersistedSnapshot>>>,
    integration_events: Arc<RwLock<Vec<SerializedIntegrationEvent>>>,
}

impl MemoryEventStore {
    pub fn new(snapshot_interval: usize) -> Self {
        Self {
            snapshot_interval,
            events: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            integration_events: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl SnapshotIntervalProvider for MemoryEventStore {
    fn snapshot_interval(&self) -> usize {
        self.snapshot_interval
    }
}

impl AggregateEventStreamer for MemoryEventStore {
    fn stream_events<T: AggregateRoot>(
        &self,
        id: &str,
        select: SequenceSelect,
    ) -> Stream<'_, SerializedDomainEvent, PersistenceError> {
        let events = self.events.read().unwrap();
        let aggregate_events = events.get(id).cloned().unwrap_or_default();

        let filtered_events: Vec<SerializedDomainEvent> = match select {
            SequenceSelect::All => aggregate_events,
            SequenceSelect::From(seq) => aggregate_events.into_iter().filter(|e| e.seq_nr >= seq).collect(),
        };

        Box::pin(stream::iter(filtered_events.into_iter().map(Ok)))
    }
}

#[async_trait]
impl Persister for MemoryEventStore {
    async fn persist(
        &self,
        domain_events: &[SerializedDomainEvent],
        integration_events: &[SerializedIntegrationEvent],
        snapshot_update: Option<&PersistedSnapshot>,
    ) -> Result<(), PersistenceError> {
        // Store domain events
        if !domain_events.is_empty() {
            let mut events = self.events.write().unwrap();
            let aggregate_id = &domain_events[0].aggregate_id;
            events
                .entry(aggregate_id.clone())
                .or_default()
                .extend(domain_events.iter().cloned());
        }

        // Store integration events
        if !integration_events.is_empty() {
            let mut int_events = self.integration_events.write().unwrap();
            int_events.extend(integration_events.iter().cloned());
        }

        // Update snapshot if provided
        if let Some(snapshot) = snapshot_update {
            let mut snapshots = self.snapshots.write().unwrap();
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
impl SnapshotGetter for MemoryEventStore {
    async fn get_snapshot<T>(&self, id: &str) -> Result<Option<PersistedSnapshot>, PersistenceError>
    where
        T: AggregateRoot,
    {
        let snapshots = self.snapshots.read().unwrap();
        Ok(snapshots.get(id).map(|s| PersistedSnapshot {
            aggregate_type: s.aggregate_type.clone(),
            aggregate_id: s.aggregate_id.clone(),
            aggregate: s.aggregate.clone(),
            seq_nr: s.seq_nr,
            version: s.version,
        }))
    }
}

/// Memory-based inverted index store for testing and development
#[derive(Clone)]
pub struct MemoryInvertedIndexStore {
    indexes: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

impl MemoryInvertedIndexStore {
    pub fn new() -> Self {
        Self {
            indexes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryInvertedIndexStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AggregateIdsLoader for MemoryInvertedIndexStore {
    async fn get_aggregate_ids(&self, keyword: &str) -> Result<Vec<String>, PersistenceError> {
        let indexes = self.indexes.read().unwrap();
        Ok(indexes
            .get(keyword)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default())
    }
}

#[async_trait]
impl InvertedIndexCommiter for MemoryInvertedIndexStore {
    async fn commit(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
        let mut indexes = self.indexes.write().unwrap();
        indexes
            .entry(keyword.to_string())
            .or_default()
            .insert(aggregate_id.to_string());
        Ok(())
    }
}

#[async_trait]
impl InvertedIndexRemover for MemoryInvertedIndexStore {
    async fn remove(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
        let mut indexes = self.indexes.write().unwrap();
        if let Some(set) = indexes.get_mut(keyword) {
            set.remove(aggregate_id);
            if set.is_empty() {
                indexes.remove(keyword);
            }
        }
        Ok(())
    }
}

/// Combined memory store that implements both EventStore and InvertedIndexStore
#[derive(Clone)]
pub struct MemoryStore {
    event_store: MemoryEventStore,
    inverted_index_store: MemoryInvertedIndexStore,
}

impl MemoryStore {
    pub fn new(snapshot_interval: usize) -> Self {
        Self {
            event_store: MemoryEventStore::new(snapshot_interval),
            inverted_index_store: MemoryInvertedIndexStore::new(),
        }
    }

    /// Get reference to the event store component
    pub fn event_store(&self) -> &MemoryEventStore {
        &self.event_store
    }

    /// Get reference to the inverted index store component
    pub fn inverted_index_store(&self) -> &MemoryInvertedIndexStore {
        &self.inverted_index_store
    }
}

// Implement all EventStore traits by delegating to event_store
impl SnapshotIntervalProvider for MemoryStore {
    fn snapshot_interval(&self) -> usize {
        self.event_store.snapshot_interval()
    }
}

impl AggregateEventStreamer for MemoryStore {
    fn stream_events<T: AggregateRoot>(
        &self,
        id: &str,
        select: SequenceSelect,
    ) -> Stream<'_, SerializedDomainEvent, PersistenceError> {
        self.event_store.stream_events::<T>(id, select)
    }
}

#[async_trait]
impl Persister for MemoryStore {
    async fn persist(
        &self,
        domain_events: &[SerializedDomainEvent],
        integration_events: &[SerializedIntegrationEvent],
        snapshot_update: Option<&PersistedSnapshot>,
    ) -> Result<(), PersistenceError> {
        self.event_store
            .persist(domain_events, integration_events, snapshot_update)
            .await
    }
}

#[async_trait]
impl SnapshotGetter for MemoryStore {
    async fn get_snapshot<T>(&self, id: &str) -> Result<Option<PersistedSnapshot>, PersistenceError>
    where
        T: AggregateRoot,
    {
        self.event_store.get_snapshot::<T>(id).await
    }
}

// Implement all InvertedIndexStore traits by delegating to inverted_index_store
#[async_trait]
impl AggregateIdsLoader for MemoryStore {
    async fn get_aggregate_ids(&self, keyword: &str) -> Result<Vec<String>, PersistenceError> {
        self.inverted_index_store.get_aggregate_ids(keyword).await
    }
}

#[async_trait]
impl InvertedIndexCommiter for MemoryStore {
    async fn commit(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
        self.inverted_index_store.commit(aggregate_id, keyword).await
    }
}

#[async_trait]
impl InvertedIndexRemover for MemoryStore {
    async fn remove(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
        self.inverted_index_store.remove(aggregate_id, keyword).await
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
        integration_event::{self, IntegrationEvent},
        message,
    };
    use serde_json::json;

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
            self.id
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

    #[tokio::test]
    async fn test_memory_event_store_basic_operations() {
        let store = MemoryEventStore::new(10);

        // Test persisting events
        let events = vec![
            SerializedDomainEvent::new(
                "evt-1".to_string(),
                "agg-1".to_string(),
                1,
                "TestAggregate".to_string(),
                "TestEvent".to_string(),
                vec![],
                json!({}),
            ),
            SerializedDomainEvent::new(
                "evt-2".to_string(),
                "agg-1".to_string(),
                2,
                "TestAggregate".to_string(),
                "TestEvent".to_string(),
                vec![],
                json!({}),
            ),
        ];

        let result = store.persist(&events, &[], None).await;
        assert!(result.is_ok());

        // Test streaming events
        use futures::StreamExt;
        let mut stream = store.stream_events::<TestAggregate>("agg-1", SequenceSelect::All);
        let mut count = 0;
        while let Some(result) = stream.next().await {
            let event = result.unwrap();
            count += 1;
            assert_eq!(event.seq_nr, count);
        }
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_memory_inverted_index_store() {
        let store = MemoryInvertedIndexStore::new();

        // Test commit
        store.commit("agg-1", "user:john").await.unwrap();
        store.commit("agg-2", "user:john").await.unwrap();
        store.commit("agg-3", "user:jane").await.unwrap();

        // Test get_aggregate_ids
        let john_aggs = store.get_aggregate_ids("user:john").await.unwrap();
        assert_eq!(john_aggs.len(), 2);
        assert!(john_aggs.contains(&"agg-1".to_string()));
        assert!(john_aggs.contains(&"agg-2".to_string()));

        // Test remove
        store.remove("agg-1", "user:john").await.unwrap();
        let john_aggs = store.get_aggregate_ids("user:john").await.unwrap();
        assert_eq!(john_aggs.len(), 1);
        assert!(john_aggs.contains(&"agg-2".to_string()));
    }

    #[tokio::test]
    async fn test_memory_store_combined() {
        let store = MemoryStore::new(5);

        // Test event store functionality
        let events = vec![SerializedDomainEvent::new(
            "evt-1".to_string(),
            "agg-1".to_string(),
            1,
            "TestAggregate".to_string(),
            "TestEvent".to_string(),
            vec![],
            json!({"test": true}),
        )];

        store.persist(&events, &[], None).await.unwrap();

        // Test inverted index functionality
        store.commit("agg-1", "type:test").await.unwrap();
        let result = store.get_aggregate_ids("type:test").await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "agg-1");

        // Test snapshot functionality
        let snapshot = PersistedSnapshot {
            aggregate_type: "TestAggregate".to_string(),
            aggregate_id: "agg-1".to_string(),
            aggregate: vec![1, 2, 3],
            seq_nr: 1,
            version: 1,
        };

        store.persist(&[], &[], Some(&snapshot)).await.unwrap();
        let retrieved = store.get_snapshot::<TestAggregate>("agg-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().version, 1);
    }

    #[tokio::test]
    async fn test_snapshot_interval_calculation() {
        let store = MemoryStore::new(10);

        // Test various snapshot interval calculations
        use crate::event_store::EventStore;
        assert_eq!(store.commit_snapshot_with_addl_events(0, 5), 0);
        assert_eq!(store.commit_snapshot_with_addl_events(0, 10), 10);
        assert_eq!(store.commit_snapshot_with_addl_events(5, 5), 5);
        assert_eq!(store.commit_snapshot_with_addl_events(8, 12), 12);
    }

    #[tokio::test]
    async fn test_integration_events() {
        let store = MemoryEventStore::new(10);

        let integration_events = vec![
            SerializedIntegrationEvent::new(
                "int-evt-1".to_string(),
                "agg-1".to_string(),
                "TestAggregate".to_string(),
                "test.event".to_string(),
                vec![],
            ),
            SerializedIntegrationEvent::new(
                "int-evt-2".to_string(),
                "agg-1".to_string(),
                "TestAggregate".to_string(),
                "test.event".to_string(),
                vec![],
            ),
        ];

        let result = store.persist(&[], &integration_events, None).await;
        assert!(result.is_ok());

        // Verify integration events were stored
        let stored_events = store.integration_events.read().unwrap();
        assert_eq!(stored_events.len(), 2);
    }

    #[tokio::test]
    async fn test_empty_keyword_removal() {
        let store = MemoryInvertedIndexStore::new();

        // Add and remove to test empty set cleanup
        store.commit("agg-1", "temp:keyword").await.unwrap();
        assert_eq!(store.get_aggregate_ids("temp:keyword").await.unwrap().len(), 1);

        store.remove("agg-1", "temp:keyword").await.unwrap();
        assert_eq!(store.get_aggregate_ids("temp:keyword").await.unwrap().len(), 0);

        // Verify the keyword was completely removed from the map
        let indexes = store.indexes.read().unwrap();
        assert!(!indexes.contains_key("temp:keyword"));
    }
}
