# Running Tests for tsuzuri-dynamodb

This directory contains integration tests for the tsuzuri-dynamodb crate.

## Prerequisites

To run the integration tests, you need LocalStack running with DynamoDB service enabled.

### Starting LocalStack

Run the following command to start LocalStack:

```bash
docker run -d \
  --name localstack \
  -p 4566:4566 \
  -e SERVICES=dynamodb \
  -e DEBUG=0 \
  localstack/localstack-pro
```

If you don't have a LocalStack Pro license, you can use the community edition:

```bash
docker run -d \
  --name localstack \
  -p 4566:4566 \
  -e SERVICES=dynamodb \
  -e DEBUG=0 \
  localstack/localstack
```

### Running Tests

Once LocalStack is running, you can run the tests:

```bash
# Run all tests
cargo test --package tsuzuri-dynamodb

# Run specific test file
cargo test --package tsuzuri-dynamodb --test event_store_test

# Run with output
cargo test --package tsuzuri-dynamodb -- --nocapture

# Run only unit tests (doesn't require LocalStack)
cargo test --package tsuzuri-dynamodb --lib
```

### Environment Variables

- `LOCALSTACK_ENDPOINT`: Override the LocalStack endpoint (default: http://localhost:4566)
- `LOCALSTACK_AUTH_TOKEN`: LocalStack Pro authentication token (if using Pro features)

### Test Structure

- `common/mod.rs`: LocalStack setup and table creation utilities
- `common/fixtures.rs`: Test fixtures including aggregate, commands, and events
- `event_store_test.rs`: Tests for event persistence and retrieval
- `inverted_index_test.rs`: Tests for keyword-based aggregate indexing
- `config_test.rs`: Tests for configuration and builder patterns

### Troubleshooting

If tests fail with connection errors:
1. Check if LocalStack is running: `docker ps`
2. Check LocalStack logs: `docker logs localstack`
3. Verify the endpoint is accessible: `curl http://localhost:4566/_localstack/health`