# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
