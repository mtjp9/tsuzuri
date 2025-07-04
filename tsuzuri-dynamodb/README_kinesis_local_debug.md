# Kinesis Local Debugger

A local debugging tool for processing Kinesis streams containing DynamoDB stream events.

## Overview

The Kinesis Local Debugger allows you to:

- Poll and process Kinesis streams locally
- Filter events by type
- Pretty-print record data
- Collect processing metrics
- Debug event processing without deploying to Lambda
- Connect to local Kinesis servers (LocalStack, etc.)

## Installation

Add `tsuzuri-dynamodb` to your `Cargo.toml`:

```toml
[dependencies]
tsuzuri-dynamodb = "0.1.23"

# For the debugger binary
[dev-dependencies]
clap = { version = "4.5", features = ["derive"] }
ctrlc = "3.4"
aws-config = "1.6"
aws-sdk-kinesis = "1.70"
tokio = { version = "1.45", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

## Creating Your Debug Binary

Create a new binary file (e.g., `src/bin/kinesis_debug.rs` or `examples/kinesis_debug.rs`):

```rust
use aws_config::BehaviorVersion;
use aws_sdk_kinesis::config::Credentials;
use clap::{Parser, ValueEnum};
use tracing::{error, info};
use tsuzuri_dynamodb::projection::{
    event_type_router::ProcessorBasedEventRouter,
    kinesis::local::{DebugConfig, LocalKinesisDebugger},
};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The name of the Kinesis stream to debug
    #[arg(short, long)]
    stream_name: String,

    /// AWS region
    #[arg(short, long, default_value = "ap-northeast-1")]
    region: String,

    /// Event types to filter (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    event_types: Option<Vec<String>>,

    /// Maximum number of records to process
    #[arg(short, long)]
    max_records: Option<usize>,

    /// Custom endpoint URL for local testing
    #[arg(long)]
    endpoint_url: Option<String>,

    /// Use local credentials for testing
    #[arg(long, default_value_t = false)]
    local_mode: bool,

    /// Log level
    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    log_level: LogLevel,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = match args.log_level {
        LogLevel::Trace => tracing::Level::TRACE,
        LogLevel::Debug => tracing::Level::DEBUG,
        LogLevel::Info => tracing::Level::INFO,
        LogLevel::Warn => tracing::Level::WARN,
        LogLevel::Error => tracing::Level::ERROR,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .init();

    // Configure AWS SDK
    let config = if args.local_mode || args.endpoint_url.is_some() {
        let mut builder = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(args.region.clone()));

        if let Some(endpoint) = args.endpoint_url.clone() {
            builder = builder.endpoint_url(endpoint);
        }

        if args.local_mode {
            let credentials = Credentials::new("test", "test", None, None, "local");
            builder = builder.credentials_provider(credentials);
        }

        builder.load().await
    } else {
        aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(args.region.clone()))
            .load()
            .await
    };

    let kinesis_client = aws_sdk_kinesis::Client::new(&config);

    // Create debug configuration
    let debug_config = DebugConfig {
        event_type_filter: args.event_types.clone(),
        max_records: args.max_records,
        pretty_print: true,
        pause_between_records: false,
        pause_duration_ms: 1000,
    };

    // Create your router with your processors
    let router = create_my_router();

    // Create and run debugger
    let debugger = LocalKinesisDebugger::new(
        kinesis_client,
        router,
        args.stream_name.clone(),
        debug_config,
    );

    info!("Starting to poll Kinesis stream...");
    debugger.run().await?;

    Ok(())
}

/// Create your custom router with your event processors
fn create_my_router() -> ProcessorBasedEventRouter {
    let router = ProcessorBasedEventRouter::new();
    
    // Register your processors here
    // Example:
    // let user_processor = Arc::new(UserEventProcessor::new(adapter));
    // router.register("UserCreated", user_processor.clone());
    // router.register("UserUpdated", user_processor);
    
    router
}
```

## Usage Examples

### Basic Usage

```bash
cargo run --bin kinesis_debug -- --stream-name my-stream
```

### With LocalStack

```bash
cargo run --bin kinesis_debug -- \
    --stream-name my-stream \
    --endpoint-url http://localhost:4566 \
    --local-mode
```

### Event Type Filtering

```bash
cargo run --bin kinesis_debug -- \
    --stream-name my-stream \
    --event-types UserCreated,UserUpdated \
    --max-records 100
```

### Specify AWS Region

```bash
cargo run --bin kinesis_debug -- \
    --stream-name my-stream \
    --region ap-northeast-1
```

## Configuration Options

### DebugConfig

- `event_type_filter`: Filter events by type (None means process all)
- `max_records`: Maximum number of records to process (None means unlimited)
- `pretty_print`: Whether to pretty-print records (default: true)
- `pause_between_records`: Whether to pause between records (default: false)
- `pause_duration_ms`: Pause duration in milliseconds (default: 1000)

## Features

### Metrics Collection

The debugger collects:

- Total records processed
- Success/failure counts
- Event type distribution
- Processing duration

### Pretty Printing

When enabled (default), the debugger displays:

- Kinesis record metadata (sequence number, partition key, arrival time)
- DynamoDB event details
- Full record JSON in a readable format

### Graceful Shutdown

Press Ctrl+C to stop the debugger gracefully. It will display a summary of the debugging session.

## Integration with Your Processors

The key to using this debugger is implementing your own `create_my_router()` function. This function should:

1. Create a `ProcessorBasedEventRouter` instance
2. Register your event processors for different event types
3. Return the configured router

Example with actual processors:

```rust
fn create_my_router() -> ProcessorBasedEventRouter {
    use tsuzuri::projection::Processor;
    use tsuzuri_dynamodb::projection::DynamoDbProjectionAdapter;
    
    let router = ProcessorBasedEventRouter::new();
    
    // Create your adapter
    let adapter = DynamoDbProjectionAdapter::new(dynamodb_client);
    
    // Create processors
    let user_processor = Arc::new(Processor::new(
        adapter.clone(),
        UserEventSerde {},
    ));
    
    let order_processor = Arc::new(Processor::new(
        adapter.clone(),
        OrderEventSerde {},
    ));
    
    // Register processors for event types
    router.register("UserCreated", user_processor.clone());
    router.register("UserUpdated", user_processor);
    router.register("OrderPlaced", order_processor);
    
    router
}
```

## AWS Credentials

The debugger uses the standard AWS SDK credential chain. Ensure you have configured:

- AWS credentials via environment variables, config files, or IAM roles
- Appropriate permissions to read from the Kinesis stream

## Troubleshooting

### No Records Received

- Verify the stream name is correct
- Check that records are being written to the stream
- Ensure your AWS credentials have permission to read from the stream
- Try using `--log-level debug` for more detailed output

### Processing Errors

- Check the event type filter matches the events in the stream
- Verify the DynamoDB stream record format is correct
- Enable debug logging to see detailed error messages
- Ensure your processors are correctly registered in the router
