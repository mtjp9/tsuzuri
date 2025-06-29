use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::config::Credentials;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType, Projection, ProjectionType,
    ScalarAttributeType,
};
use aws_sdk_dynamodb::Client;
use tsuzuri_dynamodb::store::{DynamoDB, TableNames};

pub struct LocalStackSetup {
    pub client: Client,
    pub table_names: TableNames,
    #[allow(dead_code)]
    pub endpoint_url: String,
}

impl LocalStackSetup {
    pub async fn new() -> Self {
        // For tests, we assume LocalStack is running
        // Start LocalStack with: docker run -d -p 4566:4566 localstack/localstack-pro
        let endpoint_url = std::env::var("LOCALSTACK_ENDPOINT").unwrap_or_else(|_| "http://localhost:4566".to_string());

        // Configure AWS SDK
        let config = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(&endpoint_url)
            .region(aws_config::Region::new("us-east-1"))
            .credentials_provider(Credentials::new("test", "test", None, None, "test"))
            .load()
            .await;

        let client = Client::new(&config);

        // Create test table names with random suffix to avoid conflicts
        let suffix = uuid::Uuid::new_v4().to_string().split('-').next().unwrap().to_string();
        let table_names = TableNames {
            journal: format!("test-journal-{suffix}"),
            journal_aid_index: "journal-aid-index".to_string(),
            snapshot: format!("test-snapshot-{suffix}"),
            snapshot_aid_index: "snapshot-aid-index".to_string(),
            outbox: format!("test-outbox-{suffix}"),
            outbox_status_index: "outbox-status-index".to_string(),
            inverted_index: format!("test-inverted-index-{suffix}"),
            inverted_index_keyword_index: "inverted-index-keyword-index".to_string(),
        };

        let setup = Self {
            client: client.clone(),
            table_names: table_names.clone(),
            endpoint_url,
        };

        // Create tables
        setup.create_tables().await;

        setup
    }

    async fn create_tables(&self) {
        // Create journal table
        self.create_journal_table().await;

        // Create snapshot table
        self.create_snapshot_table().await;

        // Create outbox table
        self.create_outbox_table().await;

        // Create inverted index table
        self.create_inverted_index_table().await;
    }

    async fn create_journal_table(&self) {
        let _ = self
            .client
            .create_table()
            .table_name(&self.table_names.journal)
            .billing_mode(BillingMode::PayPerRequest)
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("pkey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("skey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("aid")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("seq_nr")
                    .attribute_type(ScalarAttributeType::N)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("pkey")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("skey")
                    .key_type(KeyType::Range)
                    .build()
                    .unwrap(),
            )
            .global_secondary_indexes(
                GlobalSecondaryIndex::builder()
                    .index_name(&self.table_names.journal_aid_index)
                    .key_schema(
                        KeySchemaElement::builder()
                            .attribute_name("aid")
                            .key_type(KeyType::Hash)
                            .build()
                            .unwrap(),
                    )
                    .key_schema(
                        KeySchemaElement::builder()
                            .attribute_name("seq_nr")
                            .key_type(KeyType::Range)
                            .build()
                            .unwrap(),
                    )
                    .projection(Projection::builder().projection_type(ProjectionType::All).build())
                    .build()
                    .unwrap(),
            )
            .send()
            .await;
    }

    async fn create_snapshot_table(&self) {
        let _ = self
            .client
            .create_table()
            .table_name(&self.table_names.snapshot)
            .billing_mode(BillingMode::PayPerRequest)
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("pkey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("skey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("aid")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("seq_nr")
                    .attribute_type(ScalarAttributeType::N)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("pkey")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("skey")
                    .key_type(KeyType::Range)
                    .build()
                    .unwrap(),
            )
            .global_secondary_indexes(
                GlobalSecondaryIndex::builder()
                    .index_name(&self.table_names.snapshot_aid_index)
                    .key_schema(
                        KeySchemaElement::builder()
                            .attribute_name("aid")
                            .key_type(KeyType::Hash)
                            .build()
                            .unwrap(),
                    )
                    .key_schema(
                        KeySchemaElement::builder()
                            .attribute_name("seq_nr")
                            .key_type(KeyType::Range)
                            .build()
                            .unwrap(),
                    )
                    .projection(Projection::builder().projection_type(ProjectionType::All).build())
                    .build()
                    .unwrap(),
            )
            .send()
            .await;
    }

    async fn create_outbox_table(&self) {
        let _ = self
            .client
            .create_table()
            .table_name(&self.table_names.outbox)
            .billing_mode(BillingMode::PayPerRequest)
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("pkey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("skey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("status")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("pkey")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("skey")
                    .key_type(KeyType::Range)
                    .build()
                    .unwrap(),
            )
            .global_secondary_indexes(
                GlobalSecondaryIndex::builder()
                    .index_name(&self.table_names.outbox_status_index)
                    .key_schema(
                        KeySchemaElement::builder()
                            .attribute_name("status")
                            .key_type(KeyType::Hash)
                            .build()
                            .unwrap(),
                    )
                    .key_schema(
                        KeySchemaElement::builder()
                            .attribute_name("skey")
                            .key_type(KeyType::Range)
                            .build()
                            .unwrap(),
                    )
                    .projection(Projection::builder().projection_type(ProjectionType::All).build())
                    .build()
                    .unwrap(),
            )
            .send()
            .await;
    }

    async fn create_inverted_index_table(&self) {
        let _ = self
            .client
            .create_table()
            .table_name(&self.table_names.inverted_index)
            .billing_mode(BillingMode::PayPerRequest)
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("pkey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .attribute_definitions(
                AttributeDefinition::builder()
                    .attribute_name("skey")
                    .attribute_type(ScalarAttributeType::S)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("pkey")
                    .key_type(KeyType::Hash)
                    .build()
                    .unwrap(),
            )
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name("skey")
                    .key_type(KeyType::Range)
                    .build()
                    .unwrap(),
            )
            .send()
            .await;
    }

    pub fn create_dynamodb_store(&self) -> DynamoDB {
        DynamoDB::builder(self.client.clone())
            .table_names(self.table_names.clone())
            .shard_count(4)
            .snapshot_interval(10)
            .build()
    }
}

// Test fixtures
pub mod fixtures;
