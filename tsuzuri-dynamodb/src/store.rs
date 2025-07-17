#![forbid(unsafe_code)]
// #![deny(missing_docs)]
#![deny(clippy::all)]
#![warn(rust_2018_idioms)]

pub mod error;
pub mod helper;
pub mod key;

use crate::store::{
    error::DynamoAggregateError,
    helper::{att_as_number, att_as_vec, commit_transactions, serialized_event},
    key::{resolve_partition_key, resolve_sort_key},
};
use async_trait::async_trait;
use aws_sdk_dynamodb::{
    operation::query::{builders::QueryFluentBuilder, QueryOutput},
    primitives::Blob,
    types::{AttributeValue, Delete, Put, TransactWriteItem},
    Client,
};
use aws_smithy_types_convert::stream::PaginationStreamExt;
use futures::{Stream, StreamExt, TryStreamExt};
use std::collections::HashMap;
use tsuzuri::{
    domain_event::SerializedDomainEvent,
    event::{SequenceSelect, Stream as EventStream},
    event_store::{AggregateEventStreamer, Persister, SnapshotGetter, SnapshotIntervalProvider},
    integration_event::SerializedIntegrationEvent,
    inverted_index_store::{AggregateIdsLoader, InvertedIndexCommiter, InvertedIndexRemover},
    persist::PersistenceError,
    sequence_number::SequenceNumber,
    snapshot::PersistedSnapshot,
    AggregateRoot,
};

const OUTBOX_STATUS_PENDING: &str = "PENDING";
const OUTBOX_INITIAL_ATTEMPTS: &str = "0";

/// DynamoDB table names configuration
#[derive(Debug, Clone)]
pub struct TableNames {
    pub journal: String,
    pub journal_aid_index: String,
    pub snapshot: String,
    pub snapshot_aid_index: String,
    pub outbox: String,
    pub outbox_status_index: String,
    pub inverted_index: String,
    pub inverted_index_keyword_index: String,
}

impl Default for TableNames {
    fn default() -> Self {
        Self {
            journal: "journal".to_string(),
            journal_aid_index: "journal-aid-index".to_string(),
            snapshot: "snapshot".to_string(),
            snapshot_aid_index: "snapshot-aid-index".to_string(),
            outbox: "outbox".to_string(),
            outbox_status_index: "outbox-status-index".to_string(),
            inverted_index: "inverted-index".to_string(),
            inverted_index_keyword_index: "inverted-index-keyword-index".to_string(),
        }
    }
}

/// DynamoDB configuration
#[derive(Debug, Clone)]
pub struct DynamoDBConfig {
    pub table_names: TableNames,
    pub shard_count: usize,
    pub snapshot_interval: usize,
}

impl Default for DynamoDBConfig {
    fn default() -> Self {
        Self {
            table_names: TableNames::default(),
            shard_count: 4,
            snapshot_interval: 100,
        }
    }
}

/// Builder for DynamoDB configuration
#[derive(Debug, Default)]
pub struct DynamoDBConfigBuilder {
    table_names: Option<TableNames>,
    shard_count: Option<usize>,
    snapshot_interval: Option<usize>,
}

impl DynamoDBConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn table_names(mut self, table_names: TableNames) -> Self {
        self.table_names = Some(table_names);
        self
    }

    pub fn shard_count(mut self, count: usize) -> Self {
        self.shard_count = Some(count);
        self
    }

    pub fn snapshot_interval(mut self, interval: usize) -> Self {
        self.snapshot_interval = Some(interval);
        self
    }

    pub fn build(self) -> DynamoDBConfig {
        DynamoDBConfig {
            table_names: self.table_names.unwrap_or_default(),
            shard_count: self.shard_count.unwrap_or(4),
            snapshot_interval: self.snapshot_interval.unwrap_or(100),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DynamoDB {
    client: Client,
    config: DynamoDBConfig,
}

impl DynamoDB {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            config: DynamoDBConfig::default(),
        }
    }

    pub fn with_config(client: Client, config: DynamoDBConfig) -> Self {
        Self { client, config }
    }

    pub fn builder(client: Client) -> DynamoDBBuilder {
        DynamoDBBuilder::new(client)
    }

    pub fn table_names(&self) -> &TableNames {
        &self.config.table_names
    }

    pub fn shard_count(&self) -> usize {
        self.config.shard_count
    }

    pub fn snapshot_interval(&self) -> usize {
        self.config.snapshot_interval
    }

    fn build_all_event_transactions(
        journal_table_name: &str,
        outbox_table_name: &str,
        shard_count: usize,
        domain_events: &[SerializedDomainEvent],
        integration_events: &[SerializedIntegrationEvent],
    ) -> Result<(Vec<TransactWriteItem>, usize), DynamoAggregateError> {
        let (mut transactions, current_seq_nr) =
            Self::build_domain_event_put_transactions(journal_table_name, shard_count, domain_events)?;

        if !integration_events.is_empty() {
            let integration_transactions =
                Self::build_integration_event_put_transactions(outbox_table_name, shard_count, integration_events)?;
            transactions.extend(integration_transactions);
        }

        Ok((transactions, current_seq_nr))
    }

    fn build_domain_event_put_transactions(
        journal_table_name: &str,
        shard_count: usize,
        domain_events: &[SerializedDomainEvent],
    ) -> Result<(Vec<TransactWriteItem>, usize), DynamoAggregateError> {
        let mut current_seq_nr: usize = 0;
        let mut transactions: Vec<TransactWriteItem> = Vec::default();
        for event in domain_events {
            current_seq_nr = event.seq_nr;
            let pkey = AttributeValue::S(resolve_partition_key(
                event.aggregate_id.clone(),
                event.aggregate_type.clone(),
                shard_count,
            ));
            let skey = AttributeValue::S(resolve_sort_key(
                event.aggregate_type.clone(),
                event.aggregate_id.clone(),
                event.seq_nr,
            ));
            let event_id = AttributeValue::S(event.id.clone());
            let aid = AttributeValue::S(String::from(&event.aggregate_id));
            let seq_nr = AttributeValue::N(String::from(&event.seq_nr.to_string()));
            let aggregate_type = AttributeValue::S(String::from(&event.aggregate_type));
            let event_type = AttributeValue::S(String::from(&event.event_type));
            let payload = AttributeValue::B(Blob::new(&*event.payload));
            let metadata_blob = serde_json::to_vec(&event.metadata)?;
            let metadata = AttributeValue::B(Blob::new(metadata_blob));

            let put_event_store = Put::builder()
                .table_name(journal_table_name)
                .item("pkey", pkey.clone())
                .item("skey", skey.clone())
                .item("aid", aid)
                .item("seq_nr", seq_nr)
                .item("event_id", event_id)
                .item("aggregate_type", aggregate_type)
                .item("event_type", event_type.clone())
                .item("payload", payload.clone())
                .item("metadata", metadata.clone())
                .condition_expression("attribute_not_exists(#seq)")
                .expression_attribute_names("#seq", "seq_nr")
                .build()
                .map_err(|e| DynamoAggregateError::BuilderError(e.to_string()))?;

            let journal_item = TransactWriteItem::builder().put(put_event_store).build();
            transactions.push(journal_item);
        }
        Ok((transactions, current_seq_nr))
    }

    fn build_integration_event_put_transactions(
        outbox_table_name: &str,
        shard_count: usize,
        integration_events: &[SerializedIntegrationEvent],
    ) -> Result<Vec<TransactWriteItem>, DynamoAggregateError> {
        let mut transactions: Vec<TransactWriteItem> = Vec::default();
        for event in integration_events {
            let pkey = AttributeValue::S(resolve_partition_key(
                event.aggregate_id.clone(),
                event.aggregate_type.clone(),
                shard_count,
            ));
            let skey = AttributeValue::S(event.id.clone());
            let event_type = AttributeValue::S(String::from(&event.event_type));
            let payload = AttributeValue::B(Blob::new(&*event.payload));
            let aggregate_id = AttributeValue::S(event.aggregate_id.clone());
            let aggregate_type = AttributeValue::S(event.aggregate_type.clone());

            let put_outbox = Put::builder()
                .table_name(outbox_table_name)
                .item("pkey", pkey)
                .item("skey", skey)
                .item("aid", aggregate_id)
                .item("aggregate_type", aggregate_type)
                .item("event_type", event_type)
                .item("payload", payload)
                .item("status", AttributeValue::S(OUTBOX_STATUS_PENDING.to_string()))
                .item("attempts", AttributeValue::N(OUTBOX_INITIAL_ATTEMPTS.to_string()))
                .build()
                .map_err(|e| DynamoAggregateError::BuilderError(e.to_string()))?;
            let outbox_item = TransactWriteItem::builder().put(put_outbox).build();
            transactions.push(outbox_item);
        }
        Ok(transactions)
    }

    async fn insert_events(
        &self,
        domain_events: &[SerializedDomainEvent],
        integration_events: &[SerializedIntegrationEvent],
    ) -> Result<(), DynamoAggregateError> {
        if domain_events.is_empty() {
            return Ok(());
        }
        let (transactions, _) = Self::build_all_event_transactions(
            &self.config.table_names.journal,
            &self.config.table_names.outbox,
            self.config.shard_count,
            domain_events,
            integration_events,
        )?;
        commit_transactions(&self.client, transactions).await?;
        Ok(())
    }

    async fn query_table(
        &self,
        table: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        shard_count: usize,
        seq_nr: SequenceNumber,
    ) -> Result<QueryOutput, DynamoAggregateError> {
        let output = self
            .create_query(table, aggregate_type, aggregate_id, shard_count, seq_nr)
            .send()
            .await?;
        Ok(output)
    }

    fn create_query(
        &self,
        table: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        shard_count: usize,
        seq_nr: SequenceNumber,
    ) -> QueryFluentBuilder {
        let pkey = resolve_partition_key(aggregate_id.to_string(), aggregate_type.to_string(), shard_count);
        let skey = resolve_sort_key(aggregate_type.to_string(), aggregate_id.to_string(), seq_nr);
        self.client
            .query()
            .table_name(table)
            .consistent_read(true)
            .key_condition_expression("#pkey = :pkey AND #skey >= :skey")
            .expression_attribute_names("#pkey", "pkey")
            .expression_attribute_names("#skey", "skey")
            .expression_attribute_values(":pkey", AttributeValue::S(pkey))
            .expression_attribute_values(":skey", AttributeValue::S(skey))
    }

    async fn update_snapshot(
        &self,
        snapshot: &PersistedSnapshot,
        domain_events: &[SerializedDomainEvent],
        integration_events: &[SerializedIntegrationEvent],
    ) -> Result<(), DynamoAggregateError> {
        let expected_snapshot = snapshot.version.saturating_sub(1);
        let (mut transactions, current_seq_nr) = Self::build_all_event_transactions(
            &self.config.table_names.journal,
            &self.config.table_names.outbox,
            self.config.shard_count,
            domain_events,
            integration_events,
        )?;

        let pkey = AttributeValue::S(resolve_partition_key(
            snapshot.aggregate_id.clone(),
            snapshot.aggregate_type.clone(),
            self.config.shard_count,
        ));
        let skey = AttributeValue::S(resolve_sort_key(
            snapshot.aggregate_type.clone(),
            snapshot.aggregate_id.clone(),
            current_seq_nr,
        ));
        let aid = AttributeValue::S(String::from(&snapshot.aggregate_id));
        let current_seq_nr = AttributeValue::N(current_seq_nr.to_string());
        let version = AttributeValue::N(snapshot.version.to_string());
        let payload = AttributeValue::B(Blob::new(&*snapshot.aggregate));
        let expected_snapshot = AttributeValue::N(expected_snapshot.to_string());

        let put = Put::builder()
            .table_name(&self.config.table_names.snapshot)
            .item("pkey", pkey)
            .item("skey", skey)
            .item("aid", aid)
            .item("seq_nr", current_seq_nr)
            .item("version", version)
            .item("aggregate_type", AttributeValue::S(snapshot.aggregate_type.clone()))
            .item("payload", payload)
            .condition_expression("attribute_not_exists(version) OR (version  = :version)")
            .expression_attribute_values(":version", expected_snapshot)
            .build()
            .map_err(|e| DynamoAggregateError::BuilderError(e.to_string()))?;

        let write_item = TransactWriteItem::builder().put(put).build();
        transactions.push(write_item);
        commit_transactions(&self.client, transactions).await?;
        Ok(())
    }

    fn get_stream(
        &self,
        table_name: &str,
        table_index_name: &str,
        aggregate_id: &str,
        seq_nr: usize,
    ) -> impl Stream<Item = Result<HashMap<String, AttributeValue>, PersistenceError>> {
        self.client
            .query()
            .table_name(table_name)
            .index_name(table_index_name)
            .key_condition_expression("#aid = :aid AND #seq >= :seq")
            .expression_attribute_names("#aid", "aid")
            .expression_attribute_names("#seq", "seq_nr")
            .expression_attribute_values(":aid", AttributeValue::S(aggregate_id.to_string()))
            .expression_attribute_values(":seq", AttributeValue::N(seq_nr.to_string()))
            .consistent_read(false)
            .into_paginator()
            .items()
            .send()
            .into_stream_03x()
            .map_err(DynamoAggregateError::from)
            .map_err(PersistenceError::from)
    }

    async fn insert_inverted_index(&self, aggregate_id: &str, keyword: &str) -> Result<(), DynamoAggregateError> {
        let mut transactions: Vec<TransactWriteItem> = Vec::default();
        let pkey = AttributeValue::S(keyword.to_string());
        let skey = AttributeValue::S(aggregate_id.to_string());
        let put = Put::builder()
            .table_name(&self.config.table_names.inverted_index)
            .item("pkey", pkey.clone())
            .item("skey", skey.clone())
            .condition_expression("attribute_not_exists(pkey) AND attribute_not_exists(skey)")
            .build()
            .map_err(|e| DynamoAggregateError::BuilderError(e.to_string()))?;
        let write_item = TransactWriteItem::builder().put(put).build();
        transactions.push(write_item);
        commit_transactions(&self.client, transactions).await?;
        Ok(())
    }

    async fn query_inverted_index(&self, keyword: &str) -> Result<Vec<String>, DynamoAggregateError> {
        let response = self
            .client
            .query()
            .table_name(&self.config.table_names.inverted_index)
            .key_condition_expression("pkey = :keyword")
            .expression_attribute_values(":keyword", AttributeValue::S(keyword.to_string()))
            .send()
            .await?;
        let items = response.items.unwrap_or_default();
        let targets: Vec<String> = items
            .iter()
            .filter_map(|item| item.get("skey")?.as_s().ok().cloned())
            .collect();
        Ok(targets)
    }

    async fn remove_inverted_index(&self, aggregate_id: &str, keyword: &str) -> Result<(), DynamoAggregateError> {
        let mut transactions: Vec<TransactWriteItem> = Vec::default();
        let pkey = AttributeValue::S(keyword.to_string());
        let skey = AttributeValue::S(aggregate_id.to_string());
        let delete = Delete::builder()
            .table_name(&self.config.table_names.inverted_index)
            .key("pkey", pkey)
            .key("skey", skey)
            .build()
            .map_err(|e| DynamoAggregateError::BuilderError(e.to_string()))?;
        let write_item = TransactWriteItem::builder().delete(delete).build();
        transactions.push(write_item);
        commit_transactions(&self.client, transactions).await?;
        Ok(())
    }

    async fn get_snapshot<T: AggregateRoot>(
        &self,
        id: &str,
    ) -> Result<Option<PersistedSnapshot>, DynamoAggregateError> {
        let query_output = self
            .query_table(
                &self.config.table_names.snapshot,
                T::TYPE,
                id,
                self.config.shard_count,
                0,
            )
            .await?;
        let Some(query_items_vec) = query_output.items else {
            return Ok(None);
        };
        if query_items_vec.is_empty() {
            return Ok(None);
        }
        let query_item = query_items_vec
            .last()
            .ok_or_else(|| DynamoAggregateError::MissingAttribute("No items in query result".to_string()))?;
        let aggregate = att_as_vec(query_item, "payload")?;
        let seq_nr = att_as_number(query_item, "seq_nr")?;
        let version = att_as_number(query_item, "version")?;
        let persisted_aggregate = PersistedSnapshot {
            aggregate_type: T::TYPE.to_string(),
            aggregate_id: id.to_string(),
            aggregate,
            seq_nr,
            version,
        };
        Ok(Some(persisted_aggregate))
    }
}

#[derive(Debug)]
pub struct DynamoDBBuilder {
    client: Client,
    config_builder: DynamoDBConfigBuilder,
}

impl DynamoDBBuilder {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            config_builder: DynamoDBConfigBuilder::new(),
        }
    }

    pub fn table_names(mut self, table_names: TableNames) -> Self {
        self.config_builder = self.config_builder.table_names(table_names);
        self
    }

    pub fn shard_count(mut self, count: usize) -> Self {
        self.config_builder = self.config_builder.shard_count(count);
        self
    }

    pub fn snapshot_interval(mut self, interval: usize) -> Self {
        self.config_builder = self.config_builder.snapshot_interval(interval);
        self
    }

    pub fn build(self) -> DynamoDB {
        DynamoDB {
            client: self.client,
            config: self.config_builder.build(),
        }
    }
}

impl AggregateEventStreamer for DynamoDB {
    fn stream_events<T: AggregateRoot>(
        &self,
        id: &str,
        select: SequenceSelect,
    ) -> EventStream<'_, SerializedDomainEvent, PersistenceError> {
        self.get_stream(
            &self.config.table_names.journal,
            &self.config.table_names.journal_aid_index,
            id,
            match select {
                SequenceSelect::All => 1,
                SequenceSelect::From(seq) => seq,
            },
        )
        .map(|item| item.and_then(|entry| serialized_event(entry).map_err(PersistenceError::from)))
        .boxed()
    }
}

#[async_trait]
impl SnapshotGetter for DynamoDB {
    async fn get_snapshot<T: AggregateRoot>(&self, id: &str) -> Result<Option<PersistedSnapshot>, PersistenceError> {
        self.get_snapshot::<T>(id).await.map_err(PersistenceError::from)
    }
}

#[async_trait]
impl Persister for DynamoDB {
    async fn persist(
        &self,
        domain_events: &[SerializedDomainEvent],
        integration_events: &[SerializedIntegrationEvent],
        snapshot_update: Option<&PersistedSnapshot>,
    ) -> Result<(), PersistenceError> {
        match snapshot_update {
            None => self.insert_events(domain_events, integration_events).await?,
            Some(snapshot) => {
                self.update_snapshot(snapshot, domain_events, integration_events)
                    .await?
            }
        };
        Ok(())
    }
}

impl SnapshotIntervalProvider for DynamoDB {
    fn snapshot_interval(&self) -> usize {
        self.config.snapshot_interval
    }
}

#[async_trait]
impl AggregateIdsLoader for DynamoDB {
    async fn get_aggregate_ids(&self, keyword: &str) -> Result<Vec<String>, PersistenceError> {
        let targets = self.query_inverted_index(keyword).await?;
        Ok(targets)
    }
}

#[async_trait]
impl InvertedIndexCommiter for DynamoDB {
    async fn commit(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
        self.insert_inverted_index(aggregate_id, keyword).await?;
        Ok(())
    }
}

#[async_trait]
impl InvertedIndexRemover for DynamoDB {
    async fn remove(&self, aggregate_id: &str, keyword: &str) -> Result<(), PersistenceError> {
        self.remove_inverted_index(aggregate_id, keyword).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn test_build_domain_event_put_transactions() {
        let journal_table = "test-journal";
        let shard_count = 4;

        let events = vec![
            SerializedDomainEvent {
                id: "event-1".to_string(),
                aggregate_id: "agg-1".to_string(),
                aggregate_type: "TestAggregate".to_string(),
                seq_nr: 1,
                event_type: "Created".to_string(),
                payload: vec![1, 2, 3],
                metadata: Default::default(),
            },
            SerializedDomainEvent {
                id: "event-2".to_string(),
                aggregate_id: "agg-1".to_string(),
                aggregate_type: "TestAggregate".to_string(),
                seq_nr: 2,
                event_type: "Updated".to_string(),
                payload: vec![4, 5, 6],
                metadata: Default::default(),
            },
        ];

        let result = DynamoDB::build_domain_event_put_transactions(journal_table, shard_count, &events);

        assert!(result.is_ok());
        let (transactions, current_seq_nr) = result.unwrap();
        assert_eq!(transactions.len(), 2);
        assert_eq!(current_seq_nr, 2);
    }

    #[test]
    fn test_build_integration_event_put_transactions() {
        let outbox_table = "test-outbox";
        let shard_count = 4;

        let events = vec![SerializedIntegrationEvent {
            id: "int-event-1".to_string(),
            aggregate_id: "agg-1".to_string(),
            aggregate_type: "TestAggregate".to_string(),
            event_type: "Published".to_string(),
            payload: vec![7, 8, 9],
        }];

        let result = DynamoDB::build_integration_event_put_transactions(outbox_table, shard_count, &events);

        assert!(result.is_ok());
        let transactions = result.unwrap();
        assert_eq!(transactions.len(), 1);
    }

    #[test]
    fn test_build_all_event_transactions() {
        let journal_table = "test-journal";
        let outbox_table = "test-outbox";
        let shard_count = 4;

        let domain_events = vec![SerializedDomainEvent {
            id: "event-1".to_string(),
            aggregate_id: "agg-1".to_string(),
            aggregate_type: "TestAggregate".to_string(),
            seq_nr: 1,
            event_type: "Created".to_string(),
            payload: vec![1, 2, 3],
            metadata: Default::default(),
        }];

        let integration_events = vec![SerializedIntegrationEvent {
            id: "int-event-1".to_string(),
            aggregate_id: "agg-1".to_string(),
            aggregate_type: "TestAggregate".to_string(),
            event_type: "Published".to_string(),
            payload: vec![7, 8, 9],
        }];

        let result = DynamoDB::build_all_event_transactions(
            journal_table,
            outbox_table,
            shard_count,
            &domain_events,
            &integration_events,
        );

        assert!(result.is_ok());
        let (transactions, current_seq_nr) = result.unwrap();
        assert_eq!(transactions.len(), 2); // 1 domain + 1 integration
        assert_eq!(current_seq_nr, 1);
    }

    #[test]
    fn test_build_all_event_transactions_no_integration_events() {
        let journal_table = "test-journal";
        let outbox_table = "test-outbox";
        let shard_count = 4;

        let domain_events = vec![SerializedDomainEvent {
            id: "event-1".to_string(),
            aggregate_id: "agg-1".to_string(),
            aggregate_type: "TestAggregate".to_string(),
            seq_nr: 1,
            event_type: "Created".to_string(),
            payload: vec![1, 2, 3],
            metadata: Default::default(),
        }];

        let integration_events = vec![];

        let result = DynamoDB::build_all_event_transactions(
            journal_table,
            outbox_table,
            shard_count,
            &domain_events,
            &integration_events,
        );

        assert!(result.is_ok());
        let (transactions, current_seq_nr) = result.unwrap();
        assert_eq!(transactions.len(), 1); // Only domain event
        assert_eq!(current_seq_nr, 1);
    }
}
