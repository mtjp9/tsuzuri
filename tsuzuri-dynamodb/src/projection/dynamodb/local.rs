use crate::{
    error::{Result, StreamProcessorError},
    projection::{
        event_type_router::ProcessorBasedEventRouter,
        helpers::{extract_binary_attribute, extract_string_attribute},
    },
};
use aws_sdk_dynamodbstreams::{
    types::{Record, ShardIteratorType, StreamDescription},
    Client as DynamoDbStreamsClient,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};

/// Local DynamoDB Streams debugger for testing and debugging stream events
pub struct LocalDynamoDbStreamsDebugger {
    streams_client: DynamoDbStreamsClient,
    router: Arc<ProcessorBasedEventRouter>,
    stream_arn: String,
    metrics: Arc<Mutex<DebugMetrics>>,
    config: DebugConfig,
}

/// Configuration for the local debugger
#[derive(Clone, Debug)]
pub struct DebugConfig {
    /// Filter events by type (None means process all)
    pub event_type_filter: Option<Vec<String>>,
    /// Maximum number of records to process (None means unlimited)
    pub max_records: Option<usize>,
    /// Whether to pretty-print records
    pub pretty_print: bool,
    /// Whether to pause between records for inspection
    pub pause_between_records: bool,
    /// Pause duration in milliseconds
    pub pause_duration_ms: u64,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            event_type_filter: None,
            max_records: None,
            pretty_print: true,
            pause_between_records: false,
            pause_duration_ms: 1000,
        }
    }
}

/// Metrics collected during debugging
#[derive(Default, Debug)]
pub struct DebugMetrics {
    pub total_records: usize,
    pub processed_records: usize,
    pub failed_records: usize,
    pub event_type_counts: HashMap<String, usize>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

impl LocalDynamoDbStreamsDebugger {
    /// Create a new local DynamoDB Streams debugger
    pub fn new(
        streams_client: DynamoDbStreamsClient,
        router: ProcessorBasedEventRouter,
        stream_arn: String,
        config: DebugConfig,
    ) -> Self {
        Self {
            streams_client,
            router: Arc::new(router),
            stream_arn,
            metrics: Arc::new(Mutex::new(DebugMetrics::default())),
            config,
        }
    }

    /// Start polling and processing DynamoDB stream
    pub async fn run(&self) -> Result<()> {
        info!(
            "Starting local DynamoDB Streams debugger for stream: {}",
            self.stream_arn
        );
        info!("Config: {:?}", self.config);

        // Set start time
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.start_time = Some(Utc::now());
        }

        let max_items = self.config.max_records.unwrap_or(usize::MAX);

        let result = self.process_stream(max_items).await;

        // Set end time and print summary
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.end_time = Some(Utc::now());
        }

        self.print_summary();

        result
    }

    /// Process DynamoDB stream
    async fn process_stream(&self, max_item_count: usize) -> Result<()> {
        let stream_description = self.describe_stream().await?;
        let shards = stream_description.shards.unwrap_or_default();

        let mut total_processed = 0;

        for shard in shards {
            if total_processed >= max_item_count {
                break;
            }

            let shard_id = shard
                .shard_id
                .as_ref()
                .ok_or_else(|| StreamProcessorError::InvalidData("Shard ID not found".to_string()))?;

            let remaining = max_item_count - total_processed;
            let processed = self.process_shard(shard_id, remaining).await?;
            total_processed += processed;
        }

        Ok(())
    }

    /// Describe the stream
    async fn describe_stream(&self) -> Result<StreamDescription> {
        let resp = self
            .streams_client
            .describe_stream()
            .stream_arn(&self.stream_arn)
            .send()
            .await
            .map_err(|e| StreamProcessorError::DynamoDbStreams(format!("Failed to describe stream: {e}")))?;

        resp.stream_description
            .ok_or_else(|| StreamProcessorError::InvalidData("Stream description not found".to_string()))
    }

    /// Process a single shard
    async fn process_shard(&self, shard_id: &str, max_items: usize) -> Result<usize> {
        let shard_iterator = self.get_shard_iterator(shard_id).await?;

        let mut current_iterator = Some(shard_iterator);
        let mut processed_count = 0;

        while let Some(iterator) = current_iterator {
            if processed_count >= max_items {
                break;
            }

            let records_output = self
                .streams_client
                .get_records()
                .shard_iterator(iterator)
                .send()
                .await
                .map_err(|e| StreamProcessorError::DynamoDbStreams(format!("Failed to get records from shard: {e}")))?;

            let records = records_output.records.unwrap_or_default();
            debug!("Retrieved {} records from shard {}", records.len(), shard_id);

            for record in &records {
                if processed_count >= max_items {
                    break;
                }
                self.process_record(record).await?;
                processed_count += 1;
            }

            current_iterator = records_output.next_shard_iterator;

            // If no records, add a small delay to avoid tight polling
            if records.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        Ok(processed_count)
    }

    /// Get shard iterator
    async fn get_shard_iterator(&self, shard_id: &str) -> Result<String> {
        let output = self
            .streams_client
            .get_shard_iterator()
            .stream_arn(&self.stream_arn)
            .shard_id(shard_id)
            .shard_iterator_type(ShardIteratorType::Latest)
            .send()
            .await
            .map_err(|e| StreamProcessorError::DynamoDbStreams(format!("Failed to get shard iterator: {e}")))?;

        output
            .shard_iterator
            .ok_or_else(|| StreamProcessorError::InvalidData("No shard iterator returned".to_string()))
    }

    /// Process a single record
    async fn process_record(&self, record: &Record) -> Result<()> {
        let processor = LocalDebugProcessor {
            router: Arc::clone(&self.router),
            metrics: Arc::clone(&self.metrics),
            config: self.config.clone(),
        };
        processor.process_record(record).await
    }

    /// Print debugging summary
    fn print_summary(&self) {
        let metrics = self.metrics.lock().unwrap();

        println!("\n========== Debug Session Summary ==========");
        if let (Some(start), Some(end)) = (metrics.start_time, metrics.end_time) {
            let duration = end - start;
            println!("Duration: {duration}");
        }
        println!("Total records seen: {}", metrics.total_records);
        println!("Successfully processed: {}", metrics.processed_records);
        println!("Failed: {}", metrics.failed_records);

        if !metrics.event_type_counts.is_empty() {
            println!("\nEvent Type Distribution:");
            for (event_type, count) in &metrics.event_type_counts {
                println!("  {event_type}: {count}");
            }
        }
        println!("==========================================\n");
    }
}

/// Processor wrapper for local debugging
struct LocalDebugProcessor {
    router: Arc<ProcessorBasedEventRouter>,
    metrics: Arc<Mutex<DebugMetrics>>,
    config: DebugConfig,
}

impl LocalDebugProcessor {
    async fn process_record(&self, record: &Record) -> Result<()> {
        // Update total records count
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.total_records += 1;
        }

        // Only process INSERT and MODIFY events
        let event_name = record
            .event_name
            .as_ref()
            .ok_or_else(|| StreamProcessorError::InvalidData("Event name not found".to_string()))?
            .as_str();

        if event_name != "INSERT" && event_name != "MODIFY" {
            debug!("Skipping {} event", event_name);
            return Ok(());
        }

        // Extract the dynamodb field from the record
        let dynamodb = record
            .dynamodb
            .as_ref()
            .ok_or_else(|| StreamProcessorError::InvalidData("DynamoDB field not found".to_string()))?;

        // Get new image
        let new_image = dynamodb
            .new_image
            .as_ref()
            .ok_or_else(|| StreamProcessorError::InvalidData("New image not found".to_string()))?;

        // Convert to serde_dynamo AttributeValue format for compatibility
        let mut attribute_values = HashMap::new();
        for (key, value) in new_image {
            let dynamo_value = convert_to_serde_dynamo_value(value)?;
            attribute_values.insert(key.clone(), dynamo_value);
        }

        // Extract event type
        let event_type = match extract_string_attribute(&attribute_values, "event_type") {
            Ok(et) => et,
            Err(e) => {
                error!("Failed to extract event type: {}", e);
                let mut metrics = self.metrics.lock().unwrap();
                metrics.failed_records += 1;
                return Err(e);
            }
        };

        // Update event type metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            *metrics.event_type_counts.entry(event_type.to_string()).or_insert(0) += 1;
        }

        // Check if we should process this event type
        if let Some(ref filter) = self.config.event_type_filter {
            if !filter.contains(&event_type.to_string()) {
                debug!("Skipping event type '{}' (not in filter)", event_type);
                return Ok(());
            }
        }

        // Pretty print if enabled
        if self.config.pretty_print {
            self.pretty_print_record(record, event_name, event_type)?;
        }

        // Extract payload and metadata
        let payload_bytes = match extract_binary_attribute(&attribute_values, "payload") {
            Ok(pb) => pb,
            Err(e) => {
                error!("Failed to extract payload: {}", e);
                let mut metrics = self.metrics.lock().unwrap();
                metrics.failed_records += 1;
                return Err(e);
            }
        };
        let metadata_bytes = match extract_binary_attribute(&attribute_values, "metadata") {
            Ok(mb) => mb,
            Err(e) => {
                error!("Failed to extract metadata: {}", e);
                let mut metrics = self.metrics.lock().unwrap();
                metrics.failed_records += 1;
                return Err(e);
            }
        };

        // Process the event
        // Get sequence number from StreamRecord
        let sequence = dynamodb.sequence_number.as_deref().unwrap_or("unknown");

        info!("Processing event type '{}' with sequence {}", event_type, sequence);

        match self
            .router
            .process_bytes(event_type, &payload_bytes, &metadata_bytes)
            .await
        {
            Ok(_) => {
                info!("Successfully processed event");
                let mut metrics = self.metrics.lock().unwrap();
                metrics.processed_records += 1;
            }
            Err(e) => {
                error!("Failed to process event: {}", e);
                let mut metrics = self.metrics.lock().unwrap();
                metrics.failed_records += 1;
                return Err(StreamProcessorError::Projection(e));
            }
        }

        // Pause if configured
        if self.config.pause_between_records {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.pause_duration_ms)).await;
        }

        Ok(())
    }

    /// Pretty print a DynamoDB Streams record for debugging
    fn pretty_print_record(&self, record: &Record, event_name: &str, event_type: &str) -> Result<()> {
        println!("\n========== DynamoDB Streams Record ==========");

        // Print event details
        println!("Event Name: {event_name}");
        println!("Event Type: {event_type}");

        if let Some(event_id) = &record.event_id {
            println!("Event ID: {event_id}");
        }

        if let Some(dynamodb) = &record.dynamodb {
            // Get sequence number from StreamRecord
            if let Some(seq) = &dynamodb.sequence_number {
                println!("Sequence Number: {seq}");
            }
            if let Some(keys) = &dynamodb.keys {
                println!("Keys: {:?}", keys);
            }
            if let Some(size) = dynamodb.size_bytes {
                println!("Size: {} bytes", size);
            }
        }

        println!("============================================");

        Ok(())
    }
}

/// Convert AWS SDK DynamoDB AttributeValue to serde_dynamo AttributeValue
fn convert_to_serde_dynamo_value(
    value: &aws_sdk_dynamodbstreams::types::AttributeValue,
) -> Result<serde_dynamo::AttributeValue> {
    match value {
        aws_sdk_dynamodbstreams::types::AttributeValue::S(s) => Ok(serde_dynamo::AttributeValue::S(s.clone())),
        aws_sdk_dynamodbstreams::types::AttributeValue::N(n) => Ok(serde_dynamo::AttributeValue::N(n.clone())),
        aws_sdk_dynamodbstreams::types::AttributeValue::B(b) => {
            Ok(serde_dynamo::AttributeValue::B(b.clone().into_inner()))
        }
        aws_sdk_dynamodbstreams::types::AttributeValue::Bool(b) => Ok(serde_dynamo::AttributeValue::Bool(*b)),
        aws_sdk_dynamodbstreams::types::AttributeValue::Null(b) => Ok(serde_dynamo::AttributeValue::Null(*b)),
        aws_sdk_dynamodbstreams::types::AttributeValue::M(m) => {
            let mut map = HashMap::new();
            for (k, v) in m {
                map.insert(k.clone(), convert_to_serde_dynamo_value(v)?);
            }
            Ok(serde_dynamo::AttributeValue::M(map))
        }
        aws_sdk_dynamodbstreams::types::AttributeValue::L(l) => {
            let mut list = Vec::new();
            for item in l {
                list.push(convert_to_serde_dynamo_value(item)?);
            }
            Ok(serde_dynamo::AttributeValue::L(list))
        }
        aws_sdk_dynamodbstreams::types::AttributeValue::Ss(ss) => Ok(serde_dynamo::AttributeValue::Ss(ss.clone())),
        aws_sdk_dynamodbstreams::types::AttributeValue::Ns(ns) => Ok(serde_dynamo::AttributeValue::Ns(ns.clone())),
        aws_sdk_dynamodbstreams::types::AttributeValue::Bs(bs) => {
            let blobs: Vec<Vec<u8>> = bs.iter().map(|b| b.clone().into_inner()).collect();
            Ok(serde_dynamo::AttributeValue::Bs(blobs))
        }
        _ => Err(StreamProcessorError::InvalidData(
            "Unknown AttributeValue type".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_config_default() {
        let config = DebugConfig::default();
        assert!(config.event_type_filter.is_none());
        assert!(config.max_records.is_none());
        assert!(config.pretty_print);
        assert!(!config.pause_between_records);
        assert_eq!(config.pause_duration_ms, 1000);
    }

    #[test]
    fn test_debug_metrics_default() {
        let metrics = DebugMetrics::default();
        assert_eq!(metrics.total_records, 0);
        assert_eq!(metrics.processed_records, 0);
        assert_eq!(metrics.failed_records, 0);
        assert!(metrics.event_type_counts.is_empty());
        assert!(metrics.start_time.is_none());
        assert!(metrics.end_time.is_none());
    }
}
