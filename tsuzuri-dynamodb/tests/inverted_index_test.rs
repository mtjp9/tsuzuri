mod common;

use common::LocalStackSetup;
use tsuzuri::inverted_index_store::{AggregateIdsLoader, InvertedIndexCommiter, InvertedIndexRemover};

#[tokio::test]
async fn test_commit_and_get_aggregate_ids() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "test-agg-123";
    let keyword = "important-keyword";

    // Commit keyword to inverted index
    store
        .commit(aggregate_id, keyword)
        .await
        .expect("Failed to commit keyword");

    // Get aggregate IDs by keyword
    let ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");

    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], aggregate_id);
}

#[tokio::test]
async fn test_commit_multiple_aggregates_same_keyword() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let keyword = "shared-keyword";
    let aggregate_ids = ["agg-1", "agg-2", "agg-3"];

    // Commit same keyword for multiple aggregates
    for id in &aggregate_ids {
        store.commit(id, keyword).await.expect("Failed to commit keyword");
    }

    // Get all aggregate IDs by keyword
    let mut ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");

    ids.sort();
    let mut expected = aggregate_ids.to_vec();
    expected.sort();

    assert_eq!(ids.len(), 3);
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn test_commit_multiple_keywords_single_aggregate() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "multi-keyword-agg";
    let keywords = ["keyword1", "keyword2", "keyword3"];

    // Commit multiple keywords for single aggregate
    for keyword in &keywords {
        store
            .commit(aggregate_id, keyword)
            .await
            .expect("Failed to commit keyword");
    }

    // Verify each keyword returns the aggregate
    for keyword in &keywords {
        let ids = store
            .get_aggregate_ids(keyword)
            .await
            .expect("Failed to get aggregate IDs");

        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], aggregate_id);
    }
}

#[tokio::test]
async fn test_remove_keyword() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "removable-agg";
    let keyword = "temporary-keyword";

    // Commit keyword
    store
        .commit(aggregate_id, keyword)
        .await
        .expect("Failed to commit keyword");

    // Verify it exists
    let ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");
    assert_eq!(ids.len(), 1);

    // Remove keyword
    store
        .remove(aggregate_id, keyword)
        .await
        .expect("Failed to remove keyword");

    // Verify it's gone
    let ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");
    assert_eq!(ids.len(), 0);
}

#[tokio::test]
async fn test_remove_specific_aggregate_from_keyword() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let keyword = "shared-removable";
    let agg1 = "agg-to-keep";
    let agg2 = "agg-to-remove";

    // Commit keyword for both aggregates
    store.commit(agg1, keyword).await.expect("Failed to commit");
    store.commit(agg2, keyword).await.expect("Failed to commit");

    // Verify both exist
    let ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");
    assert_eq!(ids.len(), 2);

    // Remove only one aggregate
    store.remove(agg2, keyword).await.expect("Failed to remove keyword");

    // Verify only one remains
    let ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], agg1);
}

#[tokio::test]
async fn test_get_aggregate_ids_empty_result() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let keyword = "non-existent-keyword";

    // Get aggregate IDs for non-existent keyword
    let ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");

    assert_eq!(ids.len(), 0);
}

#[tokio::test]
async fn test_duplicate_commit_idempotent() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    let aggregate_id = "duplicate-test";
    let keyword = "duplicate-keyword";

    // Commit same keyword twice
    store
        .commit(aggregate_id, keyword)
        .await
        .expect("Failed to commit keyword");

    // This should fail because the condition expression prevents duplicates
    let result = store.commit(aggregate_id, keyword).await;
    assert!(result.is_err(), "Duplicate commit should fail");

    // Verify only one entry exists
    let ids = store
        .get_aggregate_ids(keyword)
        .await
        .expect("Failed to get aggregate IDs");
    assert_eq!(ids.len(), 1);
}

#[tokio::test]
async fn test_remove_non_existent_keyword() {
    let setup = LocalStackSetup::new().await;
    let store = setup.create_dynamodb_store();

    // Remove keyword that was never committed (should not error)
    let result = store.remove("non-existent-agg", "non-existent-keyword").await;
    assert!(result.is_ok(), "Removing non-existent keyword should not error");
}
