# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
