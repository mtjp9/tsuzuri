use serde::{Deserialize, Serialize};
use std::{fmt, marker::PhantomData, str::FromStr};
use thiserror::Error;
use ulid::Ulid;

#[derive(Debug, Error, Clone)]
pub enum AggregateIdError {
    #[error("aggregate id is empty")]
    Empty,
    #[error("aggregate id is invalid")]
    Invalid,
}

/// Trait that aggregates must implement to provide their ID prefix
pub trait HasIdPrefix: Clone + Send + Sync + 'static {
    const PREFIX: &'static str;
}

/// Generic ID structure for aggregates
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AggregateId<T: HasIdPrefix> {
    id: Ulid,
    _phantom: PhantomData<T>,
}

impl<T: HasIdPrefix> AggregateId<T> {
    pub fn new() -> Self {
        Self {
            id: Ulid::new(),
            _phantom: PhantomData,
        }
    }

    pub fn from_ulid(ulid: Ulid) -> Self {
        Self {
            id: ulid,
            _phantom: PhantomData,
        }
    }

    #[cfg(test)]
    pub fn into_inner(self) -> Ulid {
        self.id
    }
}

impl<T: HasIdPrefix> Default for AggregateId<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: HasIdPrefix> fmt::Display for AggregateId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", T::PREFIX, self.id)
    }
}

impl<T: HasIdPrefix> FromStr for AggregateId<T> {
    type Err = AggregateIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(AggregateIdError::Empty);
        }

        let ulid_string = s.strip_prefix(&format!("{}-", T::PREFIX)).unwrap_or(s);

        let ulid = Ulid::from_string(ulid_string).map_err(|_| AggregateIdError::Invalid)?;

        Ok(Self::from_ulid(ulid))
    }
}

impl<T: HasIdPrefix> Serialize for AggregateId<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de, T: HasIdPrefix> Deserialize<'de> for AggregateId<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Example implementation for ProjectId
    #[derive(Debug, Clone, PartialEq)]
    pub struct ProjectId;

    impl HasIdPrefix for ProjectId {
        const PREFIX: &'static str = "pj";
    }

    pub type ProjectIdType = AggregateId<ProjectId>;

    #[test]
    fn test_project_id_creation() {
        let id = ProjectIdType::new();
        let id_string = id.to_string();
        assert!(id_string.starts_with("pj-"));
    }

    #[test]
    fn test_project_id_from_string() {
        let id = ProjectIdType::new();
        let id_string = id.to_string();

        let parsed_id = ProjectIdType::from_str(&id_string).unwrap();
        assert_eq!(id, parsed_id);

        // Test without prefix
        let ulid_only = id.clone().into_inner().to_string();
        let parsed_id2 = ProjectIdType::from_str(&ulid_only).unwrap();
        assert_eq!(id, parsed_id2);
    }

    #[test]
    fn test_serialization() {
        let id = ProjectIdType::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: ProjectIdType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }
}
