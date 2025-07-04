use crate::config::LibSqlConfig;
use bytes::Bytes;
use libsql::{Builder, Cipher, Connection, Database, EncryptionConfig};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RemoteConfig {
    pub url: String,
    pub auth_token: String,
}

#[derive(Debug, Clone)]
pub struct EmbeddedReplicaConfig {
    pub local_path: String,
    pub sync_url: String,
    pub auth_token: String,
    pub sync_interval: Option<Duration>,
    pub encryption_key: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ConnectionConfig {
    Remote(RemoteConfig),
    EmbeddedReplica(EmbeddedReplicaConfig),
}

#[derive(Debug)]
pub enum ConnectionType {
    Remote(Connection),
    EmbeddedReplica { connection: Connection, database: Database },
}

#[derive(Debug)]
pub struct ConnectionManager {
    connection_type: ConnectionType,
}

impl ConnectionManager {
    pub async fn new(config: ConnectionConfig) -> Result<Self, libsql::Error> {
        match config {
            ConnectionConfig::Remote(remote_config) => Self::new_remote(remote_config).await,
            ConnectionConfig::EmbeddedReplica(replica_config) => Self::new_embedded_replica(replica_config).await,
        }
    }

    pub async fn from_config(config: LibSqlConfig) -> Result<Self, libsql::Error> {
        Self::new(config.connection).await
    }

    pub async fn new_remote(config: RemoteConfig) -> Result<Self, libsql::Error> {
        let db = Builder::new_remote(config.url, config.auth_token).build().await?;
        let conn = db.connect()?;
        Ok(Self {
            connection_type: ConnectionType::Remote(conn),
        })
    }

    pub async fn new_embedded_replica(config: EmbeddedReplicaConfig) -> Result<Self, libsql::Error> {
        let mut builder = Builder::new_remote_replica(config.local_path, config.sync_url, config.auth_token);

        if let Some(sync_interval) = config.sync_interval {
            builder = builder.sync_interval(sync_interval);
        }

        if let Some(encryption_key) = config.encryption_key {
            let key_bytes = if encryption_key.len() == 64 {
                // Hex encoded key (64 chars = 32 bytes)
                hex::decode(&encryption_key)
                    .map_err(|e| libsql::Error::ConnectionFailed(format!("Invalid hex in encryption key: {}", e)))?
            } else {
                // Raw string key (should be 32 bytes)
                encryption_key.into_bytes()
            };

            if key_bytes.len() != 32 {
                return Err(libsql::Error::ConnectionFailed(
                    "Encryption key must be exactly 32 bytes (256 bits) for AES-256-CBC".to_string(),
                ));
            }

            let encryption_config = EncryptionConfig::new(Cipher::Aes256Cbc, Bytes::from(key_bytes));
            builder = builder.encryption_config(encryption_config);
        }

        let db = builder.build().await?;
        let conn = db.connect()?;

        Ok(Self {
            connection_type: ConnectionType::EmbeddedReplica {
                connection: conn,
                database: db,
            },
        })
    }

    pub async fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let config = LibSqlConfig::from_env()?;
        Ok(Self::from_config(config).await?)
    }

    pub fn get_connection(&self) -> &Connection {
        match &self.connection_type {
            ConnectionType::Remote(conn) => conn,
            ConnectionType::EmbeddedReplica { connection, .. } => connection,
        }
    }

    pub async fn sync(&self) -> Result<(), libsql::Error> {
        match &self.connection_type {
            ConnectionType::Remote(_) => Ok(()),
            ConnectionType::EmbeddedReplica { database, .. } => {
                database.sync().await?;
                Ok(())
            }
        }
    }

    pub fn is_embedded_replica(&self) -> bool {
        matches!(self.connection_type, ConnectionType::EmbeddedReplica { .. })
    }
}
