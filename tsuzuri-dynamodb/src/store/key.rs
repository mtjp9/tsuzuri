use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tsuzuri::sequence_number::SequenceNumber;

pub fn resolve_partition_key(id: String, name: String, shard_count: usize) -> String {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash_value = hasher.finish();
    let remainder = hash_value % shard_count as u64;
    format!("{name}-{remainder}")
}

pub fn resolve_sort_key(name: String, id: String, seq_nr: SequenceNumber) -> String {
    format!("{name}-{id}-{seq_nr}")
}

#[cfg(test)]
mod tests {
    use super::{resolve_partition_key, resolve_sort_key};

    #[test]
    fn test_partition_key() {
        let shard_count = 4;
        let partition_key = resolve_partition_key("test".to_string(), "TestAggregate".to_string(), shard_count);
        assert_eq!(partition_key, "TestAggregate-0");
    }

    #[test]
    fn test_sort_key() {
        let seq_nr = 1;
        let sort_key = resolve_sort_key("TestAggregate".to_string(), "test".to_string(), seq_nr);
        assert_eq!(sort_key, "TestAggregate-test-1");
    }
}
