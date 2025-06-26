use serde::{Deserialize, Serialize};
use std::{fmt, marker::PhantomData, str::FromStr};
use thiserror::Error;
use ulid::Ulid;

pub trait AggregateId: Clone + fmt::Debug + fmt::Display + Send + Sync + 'static {
    const TYPE: &'static str;
}

#[derive(Debug, Error, Clone)]
pub enum AggregateIdError {
    #[error("aggregate id is empty")]
    Empty,
    #[error("aggregate id is invalid")]
    Invalid,
}

/// Marker trait for ID types
pub trait IdType: Clone + fmt::Debug + PartialEq + Send + Sync + 'static {
    const PREFIX: &'static str;
}

/// Generic ID structure that can be specialized for different aggregate types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypedId<T: IdType> {
    id: Ulid,
    _phantom: PhantomData<T>,
}

impl<T: IdType> TypedId<T> {
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

    pub fn from_string(s: &str) -> Result<Self, AggregateIdError> {
        let ulid = Ulid::from_string(s).map_err(|_| AggregateIdError::Invalid)?;
        Ok(Self::from_ulid(ulid))
    }

    pub fn into_inner(&self) -> Ulid {
        self.id
    }
}

impl<T: IdType> Default for TypedId<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: IdType> AggregateId for TypedId<T> {
    const TYPE: &'static str = T::PREFIX;
}

impl<T: IdType> fmt::Display for TypedId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", T::PREFIX, self.id)
    }
}

impl<T: IdType> FromStr for TypedId<T> {
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

impl<T: IdType> Serialize for TypedId<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de, T: IdType> Deserialize<'de> for TypedId<T> {
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
    pub struct ProjectIdType;

    impl IdType for ProjectIdType {
        const PREFIX: &'static str = "pj";
    }

    pub type ProjectId = TypedId<ProjectIdType>;

    #[test]
    fn test_project_id_creation() {
        let id = ProjectId::new();
        let id_string = id.to_string();
        assert!(id_string.starts_with("pj-"));
    }

    #[test]
    fn test_project_id_from_string() {
        let id = ProjectId::new();
        let id_string = id.to_string();

        let parsed_id = ProjectId::from_str(&id_string).unwrap();
        assert_eq!(id, parsed_id);

        // Test without prefix
        let ulid_only = id.into_inner().to_string();
        let parsed_id2 = ProjectId::from_str(&ulid_only).unwrap();
        assert_eq!(id, parsed_id2);
    }

    #[test]
    fn test_serialization() {
        let id = ProjectId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: ProjectId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }
}
