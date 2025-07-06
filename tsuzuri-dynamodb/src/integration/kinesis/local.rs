use crate::{
    error::{Result, StreamProcessorError},
    integration::{
        event_type_router::ProcessorBasedEventRouter,
        helpers::{extract_binary_attribute, extract_string_attribute},
    },
};
use aws_sdk_kinesis::{
    types::{Record, ShardIteratorType},
    Client as KinesisClient,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};

/// Local Kinesis debugger for testing and debugging DynamoDB stream events
pub struct LocalKinesisDebugger {
    kinesis_client: KinesisClient,
    router: Arc<ProcessorBasedEventRouter>,
    stream_name: String,
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

impl LocalKinesisDebugger {
    /// Create a new local Kinesis debugger
    pub fn new(
        kinesis_client: KinesisClient,
        router: ProcessorBasedEventRouter,
        stream_name: String,
        config: DebugConfig,
    ) -> Self {
        Self {
            kinesis_client,
            router: Arc::new(router),
            stream_name,
            metrics: Arc::new(Mutex::new(DebugMetrics::default())),
            config,
        }
    }

    /// Start polling and processing Kinesis stream
    pub async fn run(&self) -> Result<()> {
        info!("Starting local Kinesis debugger for stream: {}", self.stream_name);
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

    /// Process Kinesis stream
    async fn process_stream(&self, max_item_count: usize) -> Result<()> {
        let stream_description = self.describe_stream().await?;
        let shards = stream_description.shards().to_vec();

        let mut total_processed = 0;

        for shard in shards {
            if total_processed >= max_item_count {
                break;
            }

            let shard_id = shard.shard_id();
            let remaining = max_item_count - total_processed;
            let processed = self
                .process_shard(stream_description.stream_arn(), shard_id, remaining)
                .await?;
            total_processed += processed;
        }

        Ok(())
    }

    /// Describe the stream
    async fn describe_stream(&self) -> Result<aws_sdk_kinesis::types::StreamDescription> {
        let resp = self
            .kinesis_client
            .describe_stream()
            .stream_name(&self.stream_name)
            .send()
            .await
            .map_err(|e| StreamProcessorError::KinesisDataStreams(format!("Failed to describe stream: {e}")))?;

        resp.stream_description
            .ok_or_else(|| StreamProcessorError::InvalidData("Stream description not found".to_string()))
    }

    /// Process a single shard
    async fn process_shard(&self, stream_arn: &str, shard_id: &str, max_items: usize) -> Result<usize> {
        let shard_iterator = self.get_shard_iterator(stream_arn, shard_id).await?;

        let mut current_iterator = Some(shard_iterator);
        let mut processed_count = 0;

        while let Some(iterator) = current_iterator {
            if processed_count >= max_items {
                break;
            }

            let records_output = self
                .kinesis_client
                .get_records()
                .shard_iterator(iterator)
                .send()
                .await
                .map_err(|e| {
                    StreamProcessorError::KinesisDataStreams(format!("Failed to get records from shard: {e}"))
                })?;

            let records = records_output.records();
            debug!("Retrieved {} records from shard {}", records.len(), shard_id);

            for record in records {
                if processed_count >= max_items {
                    break;
                }
                self.process_record(record).await?;
                processed_count += 1;
            }

            current_iterator = records_output.next_shard_iterator().map(String::from);

            // If no records, add a small delay to avoid tight polling
            if records.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        Ok(processed_count)
    }

    /// Get shard iterator
    async fn get_shard_iterator(&self, stream_arn: &str, shard_id: &str) -> Result<String> {
        let output = self
            .kinesis_client
            .get_shard_iterator()
            .stream_arn(stream_arn)
            .shard_id(shard_id)
            .shard_iterator_type(ShardIteratorType::Latest)
            .send()
            .await
            .map_err(|e| StreamProcessorError::KinesisDataStreams(format!("Failed to get shard iterator: {e}")))?;

        output
            .shard_iterator()
            .ok_or_else(|| StreamProcessorError::InvalidData("No shard iterator returned".to_string()))
            .map(String::from)
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

        // Parse the Kinesis record data
        let data = record.data.as_ref();
        let json: serde_json::Value = serde_json::from_slice(data)
            .map_err(|e| StreamProcessorError::InvalidData(format!("Failed to deserialize payload: {e}")))?;

        // Extract DynamoDB stream record
        let stream_record: aws_lambda_events::dynamodb::StreamRecord = serde_json::from_value(json["dynamodb"].clone())
            .map_err(|e| StreamProcessorError::InvalidData(format!("Failed to convert to StreamRecord: {e}")))?;

        let attribute_values = stream_record.new_image.clone().into_inner();

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
            self.pretty_print_record(record, &json, event_type)?;
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

        // Process the event
        info!(
            "Processing event type '{}' from sequence {}",
            event_type, record.sequence_number
        );

        match self.router.process_bytes(event_type, &payload_bytes).await {
            Ok(_) => {
                info!("Successfully processed event");
                let mut metrics = self.metrics.lock().unwrap();
                metrics.processed_records += 1;
            }
            Err(e) => {
                error!("Failed to process event: {}", e);
                let mut metrics = self.metrics.lock().unwrap();
                metrics.failed_records += 1;
                return Err(StreamProcessorError::Integration(e));
            }
        }

        // Pause if configured
        if self.config.pause_between_records {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.pause_duration_ms)).await;
        }

        Ok(())
    }
}

impl LocalDebugProcessor {
    /// Pretty print a Kinesis record for debugging
    fn pretty_print_record(&self, record: &Record, json: &serde_json::Value, event_type: &str) -> Result<()> {
        println!("\n========== Kinesis Record ==========");
        println!("Sequence Number: {}", record.sequence_number);
        println!("Partition Key: {}", record.partition_key);
        if let Some(arrival) = record.approximate_arrival_timestamp {
            let arrival_time = chrono::DateTime::from_timestamp_millis(arrival.to_millis().unwrap_or(0))
                .unwrap_or_else(chrono::Utc::now);
            println!("Arrival Time: {arrival_time}");
        }
        println!("Event Type: {event_type}");

        // Print the DynamoDB event details
        if let Some(event_name) = json.get("eventName").and_then(|v| v.as_str()) {
            println!("DynamoDB Event: {event_name}");
        }

        // Pretty print the JSON
        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
            println!("Full Record:\n{pretty}");
        }

        println!("====================================");

        Ok(())
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
