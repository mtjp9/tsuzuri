# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.28] - 2025-07-04

### Changed

- Enhanced projection processor to support metadata in event processing
  - Updated `process_bytes()` method to accept metadata parameter
  - Modified `to_event()` method to deserialize and apply metadata to event envelopes
  - Updated all related tests to include metadata handling

## [0.1.27] - 2025-07-01

### Added

- Test framework module for behavior-driven development (BDD) style testing
  - `TestFramework` struct providing Given-When-Then pattern for aggregate testing
  - Fluent API for setting up test scenarios with initial events
  - Support for verifying events, errors, and aggregate state
  - Methods: `given()`, `given_no_previous_events()`, `when()`, `then_expect_events()`, `then_expect_error()`, `then_aggregate_state()`
  - Comprehensive test examples demonstrating framework usage

### Changed

- Added `PartialEq` trait derivation to domain events for test assertions
- Added `Clone` trait derivation to aggregates for test framework compatibility
- Updated aggregate module tests to use the new `TestFramework` instead of manual testing
- Improved test readability and maintainability with declarative test structure

## [0.1.26] - 2025-06-30

### Added

- Time conversion utilities for timestamp handling
  - `system_time_to_timestamp()` - Convert SystemTime to prost_types::Timestamp
  - `now_timestamp()` - Get current time as Timestamp
  - `days_from_now_timestamp()` - Get Timestamp for N days in the future
  - `to_rfc3339()` - Convert Timestamp to RFC3339 formatted string
  - `from_rfc3339()` - Parse RFC3339 string to Timestamp
  - Comprehensive unit tests for all helper functions
- Added dependencies: `prost-types` (0.13.5) and `chrono` (0.4.41)

## [0.1.25] - 2025-06-29

### Fixed

- Fixed partition key test in tsuzuri-dynamodb to expect correct hash result
  - The test was expecting "TestAggregate-1" but the hash of "test" modulo 4 correctly results in 0
  - Updated assertion to expect "TestAggregate-0"

## [0.1.24] - 2025-06-29

### Changed

- **BREAKING**: Changed `Command` trait's `id()` method signature
  - Now returns `AggregateId<Self::ID>` instead of `&AggregateId<Self::ID>`
  - All implementations must be updated to return an owned value (typically by cloning)
  - This change improves API ergonomics and consistency with other trait methods

## [0.1.23] - 2025-06-29

### Changed

- **BREAKING**: Refactored `VersionedAggregate` to remove internal event accumulation
  - Changed `handle()` method to return `Result<T::DomainEvent, T::Error>` instead of `Result<(), T::Error>`
  - Removed `take_uncommitted_events()` method
  - Removed `uncommitted_events` field from the struct
  - Simplified `VersionedAggregate` to be a pure wrapper without event accumulation

## [0.1.22] - 2025-06-29

### Changed

- Exported `EventIdType` from the public API alongside `EventId` for better type flexibility

## [0.1.21] - 2025-06-29

### Added

- Inverted index store functionality for keyword-based aggregate lookups
  - `InvertedIndexStore` trait combining loader, committer, and remover capabilities
  - `AggregateIdsLoader` trait for retrieving aggregate IDs by keyword
  - `InvertedIndexCommiter` trait for adding aggregate-keyword associations
  - `InvertedIndexRemover` trait for removing aggregate-keyword associations
- Memory-based store implementations for testing and local development
  - `MemoryEventStore` implementing all event store traits with in-memory storage
  - `MemoryInvertedIndexStore` implementing inverted index functionality
  - `MemoryStore` combining both event store and inverted index capabilities
- Batch aggregate loading with concurrency control
  - `AggregatesLoader` trait for loading multiple aggregates by keyword
  - Configurable concurrent loading limit in `EventSourced` repository
  - Default concurrent limit of 10 with `with_concurrent_limit` method for customization

### Changed

- Enhanced `Repository` trait to include `AggregatesLoader` capability
- Made `EventSourced` repository require both `EventStore` and `InvertedIndexStore`
- Improved type constraints for `IntegrationEventAdapter` and `ProjectionAdapter`
- Added `Send + Sync + 'static` bounds to repository traits for better async support
- Updated integration and projection processors to use more generic adapter constraints

## [0.1.2] - 2025-06-27

### Added

- Projection module for CQRS read model implementation
  - `Projector` trait for handling domain events and updating read models
  - `Processor` for event deserialization and projection execution
  - Dedicated error types for projection operations
  - Comprehensive test coverage
- Integration module for external event processing
  - `Executer` trait for handling integration events from external systems
  - `Processor` for integration event deserialization and processing
  - Specialized error handling for integration scenarios
  - Full test suite for integration components
- Tokio runtime dependency (v1.45.1) for enhanced async support

### Changed

- Enhanced CQRS pattern support with dedicated modules for projections and integrations

## [0.1.1] - 2025-06-27

### Changed

- Switched to reference-based serialization in the `Serializer` trait for improved performance
- Made the command module and its components (`handler`, `repository`) publicly available
- Updated minimum Rust version requirement to 1.88.0
- Applied rustfmt formatting to aggregate and persist modules

### Fixed

- Removed unnecessary clone operations throughout the codebase
- Removed redundant Clone trait bounds

## [0.1.0] - 2025-06-26

### Added

- Core event sourcing infrastructure with trait-based design
- Event store trait with streaming and persistence capabilities
- Command handler pattern with async support
- Versioned aggregates with optimistic concurrency control
- Flexible serialization framework supporting JSON and Protobuf
- Snapshot support for performance optimization
- Event-sourced repository pattern
- Comprehensive error handling for persistence operations
- Support for domain events, integration events, and messages
- Unique identifiers for aggregates and events using ULID
- Version and sequence number tracking for event ordering
