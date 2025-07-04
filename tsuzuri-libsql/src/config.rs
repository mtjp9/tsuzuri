use crate::read::{ConnectionConfig, EmbeddedReplicaConfig, RemoteConfig};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LibSqlConfig {
    pub connection: ConnectionConfig,
}

impl LibSqlConfig {
    pub fn builder() -> LibSqlConfigBuilder {
        LibSqlConfigBuilder::new()
    }

    pub fn from_remote(url: impl Into<String>, auth_token: impl Into<String>) -> Self {
        Self {
            connection: ConnectionConfig::Remote(RemoteConfig {
                url: url.into(),
                auth_token: auth_token.into(),
            }),
        }
    }

    pub fn from_embedded_replica(
        local_path: impl Into<String>,
        sync_url: impl Into<String>,
        auth_token: impl Into<String>,
    ) -> Self {
        Self {
            connection: ConnectionConfig::EmbeddedReplica(EmbeddedReplicaConfig {
                local_path: local_path.into(),
                sync_url: sync_url.into(),
                auth_token: auth_token.into(),
                sync_interval: None,
                encryption_key: None,
            }),
        }
    }

    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        use std::env;

        if env::var("DATABASE_USE_EMBEDDED_REPLICA").unwrap_or_default() == "true" {
            let config = EmbeddedReplicaConfig {
                local_path: env::var("DATABASE_LOCAL_PATH").unwrap_or_else(|_| "local.db".to_string()),
                sync_url: env::var("DATABASE_URL")?,
                auth_token: env::var("DATABASE_TOKEN")?,
                sync_interval: env::var("DATABASE_SYNC_INTERVAL_SECS")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(Duration::from_secs),
                encryption_key: env::var("DATABASE_ENCRYPTION_KEY").ok(),
            };
            Ok(Self {
                connection: ConnectionConfig::EmbeddedReplica(config),
            })
        } else {
            let config = RemoteConfig {
                url: env::var("DATABASE_URL")?,
                auth_token: env::var("DATABASE_TOKEN")?,
            };
            Ok(Self {
                connection: ConnectionConfig::Remote(config),
            })
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        match &self.connection {
            ConnectionConfig::Remote(config) => {
                if config.url.is_empty() {
                    return Err(ConfigError::InvalidConfiguration("URL cannot be empty".to_string()));
                }
                if config.auth_token.is_empty() {
                    return Err(ConfigError::InvalidConfiguration("Auth token cannot be empty".to_string()));
                }
                if !config.url.starts_with("libsql://") && !config.url.starts_with("https://") {
                    return Err(ConfigError::InvalidConfiguration(
                        "URL must start with libsql:// or https://".to_string(),
                    ));
                }
            }
            ConnectionConfig::EmbeddedReplica(config) => {
                if config.local_path.is_empty() {
                    return Err(ConfigError::InvalidConfiguration("Local path cannot be empty".to_string()));
                }
                if config.sync_url.is_empty() {
                    return Err(ConfigError::InvalidConfiguration("Sync URL cannot be empty".to_string()));
                }
                if config.auth_token.is_empty() {
                    return Err(ConfigError::InvalidConfiguration("Auth token cannot be empty".to_string()));
                }
                if !config.sync_url.starts_with("libsql://") && !config.sync_url.starts_with("https://") {
                    return Err(ConfigError::InvalidConfiguration(
                        "Sync URL must start with libsql:// or https://".to_string(),
                    ));
                }
                if let Some(ref key) = config.encryption_key {
                    let key_len = if key.len() == 64 {
                        32
                    } else {
                        key.len()
                    };
                    if key_len != 32 {
                        return Err(ConfigError::InvalidConfiguration(
                            "Encryption key must be exactly 32 bytes (256 bits)".to_string(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for LibSqlConfig {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::Remote(RemoteConfig {
                url: String::new(),
                auth_token: String::new(),
            }),
        }
    }
}

#[derive(Debug, Default)]
pub struct LibSqlConfigBuilder {
    connection_type: Option<ConnectionType>,
    url: Option<String>,
    auth_token: Option<String>,
    local_path: Option<String>,
    sync_interval: Option<Duration>,
    encryption_key: Option<String>,
}

#[derive(Debug)]
enum ConnectionType {
    Remote,
    EmbeddedReplica,
}

impl LibSqlConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn remote(mut self) -> Self {
        self.connection_type = Some(ConnectionType::Remote);
        self
    }

    pub fn embedded_replica(mut self) -> Self {
        self.connection_type = Some(ConnectionType::EmbeddedReplica);
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn local_path(mut self, path: impl Into<String>) -> Self {
        self.local_path = Some(path.into());
        self
    }

    pub fn sync_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = Some(interval);
        self
    }

    pub fn encryption_key(mut self, key: impl Into<String>) -> Self {
        self.encryption_key = Some(key.into());
        self
    }

    pub fn build(self) -> Result<LibSqlConfig, ConfigError> {
        let connection_type = self.connection_type.ok_or(ConfigError::MissingConnectionType)?;
        let url = self.url.ok_or(ConfigError::MissingUrl)?;
        let auth_token = self.auth_token.ok_or(ConfigError::MissingAuthToken)?;

        let connection = match connection_type {
            ConnectionType::Remote => ConnectionConfig::Remote(RemoteConfig { url, auth_token }),
            ConnectionType::EmbeddedReplica => {
                let local_path = self.local_path.ok_or(ConfigError::MissingLocalPath)?;
                ConnectionConfig::EmbeddedReplica(EmbeddedReplicaConfig {
                    local_path,
                    sync_url: url,
                    auth_token,
                    sync_interval: self.sync_interval,
                    encryption_key: self.encryption_key,
                })
            }
        };

        let config = LibSqlConfig { connection };
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Connection type not specified. Use .remote() or .embedded_replica()")]
    MissingConnectionType,
    #[error("URL is required")]
    MissingUrl,
    #[error("Authentication token is required")]
    MissingAuthToken,
    #[error("Local path is required for embedded replica")]
    MissingLocalPath,
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
}