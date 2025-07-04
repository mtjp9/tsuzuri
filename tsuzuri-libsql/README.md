# Tsuzuri libSQL

LibSQL connection management for Tsuzuri framework.

## Usage

### Direct Configuration

```rust
use tsuzuri_libsql::{LibSqlConfig, ConnectionManager};

// Remote connection
let config = LibSqlConfig::from_remote(
    "libsql://your-database.turso.io",
    "your-auth-token"
);
let manager = ConnectionManager::from_config(config).await?;

// Embedded replica
let config = LibSqlConfig::from_embedded_replica(
    "local.db",
    "libsql://your-database.turso.io",
    "your-auth-token"
);
let manager = ConnectionManager::from_config(config).await?;
```

### Using Builder Pattern

```rust
use tsuzuri_libsql::{LibSqlConfig, ConnectionManager};
use std::time::Duration;

// Remote connection with builder
let config = LibSqlConfig::builder()
    .remote()
    .url("libsql://your-database.turso.io")
    .auth_token("your-auth-token")
    .build()?;
let manager = ConnectionManager::from_config(config).await?;

// Embedded replica with encryption
let config = LibSqlConfig::builder()
    .embedded_replica()
    .url("libsql://your-database.turso.io")
    .auth_token("your-auth-token")
    .local_path("local.db")
    .sync_interval(Duration::from_secs(60))
    .encryption_key("your-32-byte-encryption-key")
    .build()?;
let manager = ConnectionManager::from_config(config).await?;
```

### Environment Variables

```bash
# For remote connection
export DATABASE_URL="libsql://your-database.turso.io"
export DATABASE_TOKEN="your-auth-token"

# For embedded replica
export DATABASE_USE_EMBEDDED_REPLICA="true"
export DATABASE_URL="libsql://your-database.turso.io"
export DATABASE_TOKEN="your-auth-token"
export DATABASE_LOCAL_PATH="local.db"
export DATABASE_SYNC_INTERVAL_SECS="60"
export DATABASE_ENCRYPTION_KEY="your-32-byte-encryption-key"
```

```rust
use tsuzuri_libsql::{LibSqlConfig, ConnectionManager};

// From environment variables
let config = LibSqlConfig::from_env()?;
let manager = ConnectionManager::from_config(config).await?;

// Or directly
let manager = ConnectionManager::from_env().await?;
```

### Generating Encryption Key

```bash
export DATABASE_ENCRYPTION_KEY=$(openssl rand -hex 32)
# Example: a1b2c3d4e5f67890a1b2c3d4e5f67890a1b2c3d4e5f67890a1b2c3d4e5f67890
```
