# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.23] - 2025-07-04

### Added

- Kinesis Data Streams integration for event processing
  - New `integration` and `projection` modules for stream processing
  - Event type router for processor-based event routing
  - Lambda function support for Kinesis event processing
  - Helper utilities for stream processing workflows

### Changed

- Enabled AWS Lambda and Kinesis dependencies in Cargo.toml
  - `aws_lambda_events` v0.16.0 with DynamoDB and Kinesis features
  - `lambda_runtime` v0.14.2 for Lambda execution
  - `aws-sdk-kinesis` v1.70.0 for Kinesis client
  - Re-enabled `chrono`, `http`, and `base64` dependencies

### Removed

- DynamoDB Streams error handling (replaced with Kinesis Data Streams)
  - Removed `DynamoDB` and `DynamoDBStreams` error variants
  - Simplified error structure to focus on Kinesis integration

## [0.1.22] - 2025-07-01

### Added

- Test framework examples demonstrating the new tsuzuri test framework
  - Created `test_framework_example.rs` with comprehensive test scenarios
  - Examples for aggregate creation, updates, state verification, and event sequences
  - Demonstrates Given-When-Then pattern usage with DynamoDB fixtures

### Changed

- Added `PartialEq` trait to test domain events (`TestAggregateCreated`, `TestAggregateUpdated`, `TestDomainEvent`)
- Added `#[allow(dead_code)]` attributes to test fixtures to suppress warnings
- Enhanced test fixtures compatibility with tsuzuri's new test framework

## [0.1.21] - 2025-06-29

### Changed

- Updated test fixtures to comply with tsuzuri 0.1.24's Command trait changes

## [0.1.2] - 2025-06-29

### Changed

- Made `DynamoDB` struct cloneable by deriving `Clone` trait
  - Enables sharing DynamoDB instances across multiple components or threads
  - Improves ergonomics when using the event store in concurrent contexts

## [0.1.1] - 2025-06-29

### Added

- Complete DynamoDB implementation for event sourcing
  - `DynamoDBEventStore` implementing all tsuzuri event store traits
  - Journal table for event storage with aggregate ID indexing
  - Snapshot table for performance optimization
  - Automatic event streaming with pagination support
- Inverted index store for keyword-based aggregate lookups
  - `DynamoDBInvertedIndexStore` for efficient aggregate discovery
  - Support for adding and removing keyword-aggregate associations
  - Global secondary index for keyword queries
- Outbox pattern implementation for integration events
  - Reliable event publishing with at-least-once delivery
  - Status tracking and retry mechanism support
  - Global secondary index for pending event queries
- Comprehensive error handling
  - `DynamoAggregateError` for event store operations
  - `StreamProcessorError` for stream processing
  - Integration with tsuzuri core error types
- Flexible configuration system
  - `DynamoDBConfig` with builder pattern
  - Customizable table names for all DynamoDB tables
  - Configurable shard count for partition key distribution
  - Adjustable snapshot interval for performance tuning
- Helper utilities
  - Key generation functions for partition and sort keys
  - Event serialization and deserialization helpers
  - Transaction management for atomic operations
- Comprehensive test suite
  - Integration tests with testcontainers
  - Configuration validation tests
  - Event store operation tests
  - Inverted index functionality tests

### Dependencies

- aws-sdk-dynamodb v1.80.0
- aws-config v1.6.1
- serde_dynamo v4.2.14
- testcontainers v0.24.0 (dev dependency)

## [0.1.0] - 2025-06-27

Initial release
