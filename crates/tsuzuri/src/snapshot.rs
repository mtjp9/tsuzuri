use crate::{sequence_number::SequenceNumber, version::Version};

#[derive(Debug, PartialEq, Eq)]
pub struct PersistedSnapshot {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub aggregate: Vec<u8>,
    pub seq_nr: SequenceNumber,
    pub version: Version,
}

impl PersistedSnapshot {
    pub fn new(
        aggregate_type: String,
        aggregate_id: String,
        aggregate: Vec<u8>,
        seq_nr: SequenceNumber,
        version: Version,
    ) -> Self {
        Self {
            aggregate_type,
            aggregate_id,
            aggregate,
            seq_nr,
            version,
        }
    }
}
