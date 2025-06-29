use crate::persist::PersistenceError;
use async_trait::async_trait;

pub trait InvertedIndexStore:
    AggregateIdsLoader + InvertedIndexCommiter + InvertedIndexRemover + Send + Sync + 'static
{
}

/// A marker trait for types that can be used as an event store.
impl<T> InvertedIndexStore for T where
    T: AggregateIdsLoader + InvertedIndexCommiter + InvertedIndexRemover + Send + Sync + 'static
{
}

#[async_trait]
pub trait AggregateIdsLoader: Send + Sync + 'static {
    async fn get_aggregate_ids(&self, keyword: &str) -> Result<Vec<String>, PersistenceError>;
}

#[async_trait]
pub trait InvertedIndexCommiter: Send + Sync + 'static {
    async fn commit(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError>;
}

#[async_trait]
pub trait InvertedIndexRemover: Send + Sync + 'static {
    async fn remove(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockInvertedIndexStore {
        indexes: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    }

    impl MockInvertedIndexStore {
        fn new() -> Self {
            Self {
                indexes: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl AggregateIdsLoader for MockInvertedIndexStore {
        async fn get_aggregate_ids(&self, keyword: &str) -> Result<Vec<String>, PersistenceError> {
            let indexes = self.indexes.lock().unwrap();
            Ok(indexes
                .get(keyword)
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default())
        }
    }

    #[async_trait]
    impl InvertedIndexCommiter for MockInvertedIndexStore {
        async fn commit(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
            let mut indexes = self.indexes.lock().unwrap();
            indexes
                .entry(keyword.to_string())
                .or_default()
                .insert(aggregate_id.to_string());
            Ok(())
        }
    }

    #[async_trait]
    impl InvertedIndexRemover for MockInvertedIndexStore {
        async fn remove(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
            let mut indexes = self.indexes.lock().unwrap();
            if let Some(set) = indexes.get_mut(keyword) {
                set.remove(aggregate_id);
                if set.is_empty() {
                    indexes.remove(keyword);
                }
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_aggregate_ids_loader() {
        let store = MockInvertedIndexStore::new();

        // Test empty result
        let result = store.get_aggregate_ids("non-existent").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn test_inverted_index_commiter() {
        let store = MockInvertedIndexStore::new();

        // Test commit
        let result = store.commit("agg-1", "user:john").await;
        assert!(result.is_ok());

        // Verify data was stored
        let indexes = store.indexes.lock().unwrap();
        assert!(indexes.contains_key("user:john"));
        assert!(indexes.get("user:john").unwrap().contains("agg-1"));
    }

    #[tokio::test]
    async fn test_inverted_index_remover() {
        let store = MockInvertedIndexStore::new();

        // Setup test data
        store.commit("agg-1", "user:john").await.unwrap();

        // Test remove
        let result = store.remove("agg-1", "user:john").await;
        assert!(result.is_ok());

        // Verify data was removed
        let indexes = store.indexes.lock().unwrap();
        assert!(!indexes.contains_key("user:john"));
    }

    #[tokio::test]
    async fn test_commit_and_get() {
        let store = MockInvertedIndexStore::new();

        // Commit multiple aggregates
        store.commit("agg-1", "user:john").await.unwrap();
        store.commit("agg-2", "user:john").await.unwrap();
        store.commit("agg-3", "user:jane").await.unwrap();

        // Test get for user:john
        let result = store.get_aggregate_ids("user:john").await.unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"agg-1".to_string()));
        assert!(result.contains(&"agg-2".to_string()));

        // Test get for user:jane
        let result = store.get_aggregate_ids("user:jane").await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&"agg-3".to_string()));
    }

    #[tokio::test]
    async fn test_remove_and_get() {
        let store = MockInvertedIndexStore::new();

        // Setup test data
        store.commit("agg-1", "user:john").await.unwrap();
        store.commit("agg-2", "user:john").await.unwrap();

        // Remove one aggregate
        store.remove("agg-1", "user:john").await.unwrap();

        // Verify only agg-2 remains
        let result = store.get_aggregate_ids("user:john").await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&"agg-2".to_string()));

        // Remove the last aggregate
        store.remove("agg-2", "user:john").await.unwrap();

        // Verify empty result
        let result = store.get_aggregate_ids("user:john").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_keywords() {
        let store = MockInvertedIndexStore::new();

        // Commit same aggregate with multiple keywords
        store.commit("agg-1", "user:john").await.unwrap();
        store.commit("agg-1", "status:active").await.unwrap();
        store.commit("agg-1", "tag:important").await.unwrap();

        // Verify all keywords work
        assert!(!store.get_aggregate_ids("user:john").await.unwrap().is_empty());
        assert!(!store.get_aggregate_ids("status:active").await.unwrap().is_empty());
        assert!(!store.get_aggregate_ids("tag:important").await.unwrap().is_empty());

        // Remove from one keyword
        store.remove("agg-1", "user:john").await.unwrap();

        // Verify other keywords still work
        assert!(store.get_aggregate_ids("user:john").await.unwrap().is_empty());
        assert!(!store.get_aggregate_ids("status:active").await.unwrap().is_empty());
        assert!(!store.get_aggregate_ids("tag:important").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_multiple_aggregates() {
        let store = MockInvertedIndexStore::new();

        // Commit multiple aggregates with same keyword
        for i in 1..=5 {
            store.commit(&format!("agg-{i}"), "tag:test").await.unwrap();
        }

        // Verify all aggregates are returned
        let result = store.get_aggregate_ids("tag:test").await.unwrap();
        assert_eq!(result.len(), 5);

        // Remove some aggregates
        store.remove("agg-2", "tag:test").await.unwrap();
        store.remove("agg-4", "tag:test").await.unwrap();

        // Verify remaining aggregates
        let result = store.get_aggregate_ids("tag:test").await.unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"agg-1".to_string()));
        assert!(result.contains(&"agg-3".to_string()));
        assert!(result.contains(&"agg-5".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let store = MockInvertedIndexStore::new();
        let store_clone1 = store.clone();
        let store_clone2 = store.clone();

        // Concurrent commits
        let handle1 = tokio::spawn(async move {
            for i in 0..10 {
                store_clone1.commit(&format!("agg-{i}"), "concurrent").await.unwrap();
            }
        });

        let handle2 = tokio::spawn(async move {
            for i in 10..20 {
                store_clone2.commit(&format!("agg-{i}"), "concurrent").await.unwrap();
            }
        });

        handle1.await.unwrap();
        handle2.await.unwrap();

        // Verify all aggregates were committed
        let result = store.get_aggregate_ids("concurrent").await.unwrap();
        assert_eq!(result.len(), 20);
    }
}
