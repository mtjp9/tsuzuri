use aws_sdk_dynamodb::Client;
use tsuzuri_dynamodb::store::{DynamoDB, DynamoDBConfig, DynamoDBConfigBuilder, TableNames};

fn create_mock_client() -> Client {
    // This creates a client but we won't actually use it for these tests
    let config = aws_sdk_dynamodb::Config::builder()
        .behavior_version(aws_sdk_dynamodb::config::BehaviorVersion::latest())
        .endpoint_url("http://localhost:4566")
        .region(aws_sdk_dynamodb::config::Region::new("us-east-1"))
        .credentials_provider(aws_sdk_dynamodb::config::Credentials::new(
            "test", "test", None, None, "test",
        ))
        .build();
    Client::from_conf(config)
}

#[test]
fn test_table_names_default() {
    let table_names = TableNames::default();

    assert_eq!(table_names.journal, "journal");
    assert_eq!(table_names.journal_aid_index, "journal-aid-index");
    assert_eq!(table_names.snapshot, "snapshot");
    assert_eq!(table_names.snapshot_aid_index, "snapshot-aid-index");
    assert_eq!(table_names.outbox, "outbox");
    assert_eq!(table_names.outbox_status_index, "outbox-status-index");
    assert_eq!(table_names.inverted_index, "inverted-index");
    assert_eq!(table_names.inverted_index_keyword_index, "inverted-index-keyword-index");
}

#[test]
fn test_dynamodb_config_default() {
    let config = DynamoDBConfig::default();

    assert_eq!(config.shard_count, 4);
    assert_eq!(config.snapshot_interval, 100);

    // Table names should also be default
    assert_eq!(config.table_names.journal, "journal");
}

#[test]
fn test_dynamodb_config_builder() {
    let custom_table_names = TableNames {
        journal: "custom-journal".to_string(),
        journal_aid_index: "custom-journal-index".to_string(),
        snapshot: "custom-snapshot".to_string(),
        snapshot_aid_index: "custom-snapshot-index".to_string(),
        outbox: "custom-outbox".to_string(),
        outbox_status_index: "custom-outbox-index".to_string(),
        inverted_index: "custom-inverted".to_string(),
        inverted_index_keyword_index: "custom-inverted-index".to_string(),
    };

    let config = DynamoDBConfigBuilder::new()
        .table_names(custom_table_names.clone())
        .shard_count(8)
        .snapshot_interval(50)
        .build();

    assert_eq!(config.shard_count, 8);
    assert_eq!(config.snapshot_interval, 50);
    assert_eq!(config.table_names.journal, "custom-journal");
    assert_eq!(config.table_names.snapshot, "custom-snapshot");
}

#[test]
fn test_dynamodb_config_builder_partial() {
    // Test with only some fields set
    let config = DynamoDBConfigBuilder::new().shard_count(16).build();

    assert_eq!(config.shard_count, 16);
    assert_eq!(config.snapshot_interval, 100); // Default value
    assert_eq!(config.table_names.journal, "journal"); // Default table names
}

#[test]
fn test_dynamodb_new() {
    let client = create_mock_client();
    let db = DynamoDB::new(client.clone());

    assert_eq!(db.shard_count(), 4);
    assert_eq!(db.snapshot_interval(), 100);
    assert_eq!(db.table_names().journal, "journal");
}

#[test]
fn test_dynamodb_with_config() {
    let client = create_mock_client();
    let config = DynamoDBConfig {
        table_names: TableNames {
            journal: "test-journal".to_string(),
            ..TableNames::default()
        },
        shard_count: 10,
        snapshot_interval: 200,
    };

    let db = DynamoDB::with_config(client, config);

    assert_eq!(db.shard_count(), 10);
    assert_eq!(db.snapshot_interval(), 200);
    assert_eq!(db.table_names().journal, "test-journal");
}

#[test]
fn test_dynamodb_builder() {
    let client = create_mock_client();
    let custom_tables = TableNames {
        journal: "builder-journal".to_string(),
        journal_aid_index: "builder-journal-index".to_string(),
        snapshot: "builder-snapshot".to_string(),
        snapshot_aid_index: "builder-snapshot-index".to_string(),
        outbox: "builder-outbox".to_string(),
        outbox_status_index: "builder-outbox-index".to_string(),
        inverted_index: "builder-inverted".to_string(),
        inverted_index_keyword_index: "builder-inverted-index".to_string(),
    };

    let db = DynamoDB::builder(client)
        .table_names(custom_tables)
        .shard_count(12)
        .snapshot_interval(150)
        .build();

    assert_eq!(db.shard_count(), 12);
    assert_eq!(db.snapshot_interval(), 150);
    assert_eq!(db.table_names().journal, "builder-journal");
    assert_eq!(db.table_names().outbox, "builder-outbox");
}

#[test]
fn test_dynamodb_builder_chain() {
    let client = create_mock_client();

    // Test method chaining
    let db = DynamoDB::builder(client).shard_count(20).snapshot_interval(300).build();

    assert_eq!(db.shard_count(), 20);
    assert_eq!(db.snapshot_interval(), 300);
}

#[test]
fn test_table_names_clone() {
    let original = TableNames {
        journal: "cloned-journal".to_string(),
        ..TableNames::default()
    };

    let cloned = original.clone();

    assert_eq!(cloned.journal, "cloned-journal");
    assert_eq!(cloned.snapshot, "snapshot");
}

#[test]
fn test_dynamodb_config_clone() {
    let original = DynamoDBConfig {
        table_names: TableNames {
            journal: "config-journal".to_string(),
            ..TableNames::default()
        },
        shard_count: 6,
        snapshot_interval: 75,
    };

    let cloned = original.clone();

    assert_eq!(cloned.shard_count, 6);
    assert_eq!(cloned.snapshot_interval, 75);
    assert_eq!(cloned.table_names.journal, "config-journal");
}
