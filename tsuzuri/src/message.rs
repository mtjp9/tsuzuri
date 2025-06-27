use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub trait Message {
    fn name(&self) -> &'static str;
}

pub type Metadata = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T>
where
    T: Message,
{
    pub message: T,
    pub metadata: Metadata,
}

impl<T> Envelope<T>
where
    T: Message,
{
    #[must_use]
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    #[must_use]
    pub fn set_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl<T> From<T> for Envelope<T>
where
    T: Message,
{
    fn from(message: T) -> Self {
        Envelope {
            message,
            metadata: Metadata::default(),
        }
    }
}

impl<T> PartialEq for Envelope<T>
where
    T: Message + PartialEq,
{
    fn eq(&self, other: &Envelope<T>) -> bool {
        self.message == other.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct StringMessage(pub(crate) &'static str);

    impl Message for StringMessage {
        fn name(&self) -> &'static str {
            "string_payload"
        }
    }

    #[test]
    fn message_with_metadata_does_not_affect_equality() {
        let message = Envelope {
            message: StringMessage("hello"),
            metadata: Metadata::default(),
        };

        let new_message = message
            .clone()
            .with_metadata("hello_world".into(), "test".into())
            .with_metadata("test_number".into(), 1.to_string());

        assert_eq!(message, new_message);
    }
}
