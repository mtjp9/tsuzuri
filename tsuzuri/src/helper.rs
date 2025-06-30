use chrono::{DateTime, Utc};
use prost_types::{Timestamp, TimestampError};
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

/// Convert a `prost_types::Timestamp` to a `SystemTime`.
pub fn system_time_to_timestamp(time: SystemTime) -> Result<Timestamp, String> {
    time.duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Time conversion error: {e}"))
        .map(|duration| Timestamp {
            seconds: duration.as_secs() as i64,
            nanos: duration.subsec_nanos() as i32,
        })
}

/// Convert a `SystemTime` to a `prost_types::Timestamp`.
pub fn now_timestamp() -> Option<Timestamp> {
    system_time_to_timestamp(SystemTime::now()).ok()
}

/// Convert a `SystemTime` to a `prost_types::Timestamp`.
pub fn days_from_now_timestamp(days: u64) -> Option<Timestamp> {
    let now = SystemTime::now();
    let duration = std::time::Duration::from_secs(60 * 60 * 24 * days);
    let future_time = now.checked_add(duration)?;
    system_time_to_timestamp(future_time).ok()
}

/// Convert a `prost_types::Timestamp` to an RFC3339 formatted string.
pub fn to_rfc3339(ts: &Timestamp) -> Result<String, TimestampError> {
    let system_time: SystemTime = (*ts).try_into()?;
    let dt: DateTime<Utc> = DateTime::<Utc>::from(system_time);
    Ok(dt.to_rfc3339())
}

/// Convert a string in RFC3339 format to a `prost_types::Timestamp`.
pub fn from_rfc3339(s: &str) -> Result<Timestamp, String> {
    let dt = DateTime::parse_from_rfc3339(s).map_err(|e| format!("Failed to parse RFC3339 string: {e}"))?;
    let system_time = SystemTime::from(dt);
    system_time_to_timestamp(system_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_system_time_to_timestamp() {
        // Test with UNIX_EPOCH
        let result = system_time_to_timestamp(UNIX_EPOCH);
        assert!(result.is_ok());
        let timestamp = result.unwrap();
        assert_eq!(timestamp.seconds, 0);
        assert_eq!(timestamp.nanos, 0);

        // Test with a specific time
        let specific_time = UNIX_EPOCH + Duration::from_secs(1000) + Duration::from_nanos(123456789);
        let result = system_time_to_timestamp(specific_time);
        assert!(result.is_ok());
        let timestamp = result.unwrap();
        assert_eq!(timestamp.seconds, 1000);
        assert_eq!(timestamp.nanos, 123456789);
    }

    #[test]
    fn test_now_timestamp() {
        let before = SystemTime::now();
        let timestamp = now_timestamp();
        let after = SystemTime::now();

        assert!(timestamp.is_some());
        let ts = timestamp.unwrap();

        // Verify the timestamp is within the expected range
        let before_duration = before.duration_since(UNIX_EPOCH).unwrap();
        let after_duration = after.duration_since(UNIX_EPOCH).unwrap();

        assert!(ts.seconds >= before_duration.as_secs() as i64);
        assert!(ts.seconds <= after_duration.as_secs() as i64);
    }

    #[test]
    fn test_days_from_now_timestamp() {
        // Test with 0 days
        let now = now_timestamp().unwrap();
        let zero_days = days_from_now_timestamp(0).unwrap();
        assert!(zero_days.seconds >= now.seconds);
        assert!(zero_days.seconds - now.seconds < 1); // Should be within 1 second

        // Test with 1 day
        let one_day = days_from_now_timestamp(1).unwrap();
        let expected_diff = 60 * 60 * 24; // seconds in a day
        assert!(one_day.seconds - now.seconds >= expected_diff - 1);
        assert!(one_day.seconds - now.seconds <= expected_diff + 1);

        // Test with 7 days
        let seven_days = days_from_now_timestamp(7).unwrap();
        let expected_diff = 60 * 60 * 24 * 7; // seconds in 7 days
        assert!(seven_days.seconds - now.seconds >= expected_diff - 1);
        assert!(seven_days.seconds - now.seconds <= expected_diff + 1);
    }

    #[test]
    fn test_to_rfc3339() {
        // Test with epoch
        let timestamp = Timestamp { seconds: 0, nanos: 0 };
        let result = to_rfc3339(&timestamp);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1970-01-01T00:00:00+00:00");

        // Test with specific timestamp
        let timestamp = Timestamp {
            seconds: 1609459200, // 2021-01-01 00:00:00 UTC
            nanos: 0,
        };
        let result = to_rfc3339(&timestamp);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "2021-01-01T00:00:00+00:00");

        // Test with nanoseconds
        let timestamp = Timestamp {
            seconds: 1609459200,
            nanos: 123456789,
        };
        let result = to_rfc3339(&timestamp);
        assert!(result.is_ok());
        let rfc3339 = result.unwrap();
        assert!(rfc3339.starts_with("2021-01-01T00:00:00.123456789"));
    }

    #[test]
    fn test_from_rfc3339() {
        // Test with valid RFC3339 string
        let rfc3339 = "2021-01-01T00:00:00Z";
        let result = from_rfc3339(rfc3339);
        assert!(result.is_ok());
        let timestamp = result.unwrap();
        assert_eq!(timestamp.seconds, 1609459200);
        assert_eq!(timestamp.nanos, 0);

        // Test with timezone offset
        let rfc3339 = "2021-01-01T00:00:00+00:00";
        let result = from_rfc3339(rfc3339);
        assert!(result.is_ok());
        let timestamp = result.unwrap();
        assert_eq!(timestamp.seconds, 1609459200);

        // Test with nanoseconds
        let rfc3339 = "2021-01-01T00:00:00.123456789Z";
        let result = from_rfc3339(rfc3339);
        assert!(result.is_ok());
        let timestamp = result.unwrap();
        assert_eq!(timestamp.seconds, 1609459200);
        assert_eq!(timestamp.nanos, 123456789);

        // Test with invalid string
        let invalid = "not a valid date";
        let result = from_rfc3339(invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse RFC3339 string"));
    }

    #[test]
    fn test_roundtrip_conversion() {
        // Test that converting to RFC3339 and back gives the same timestamp
        let original = Timestamp {
            seconds: 1609459200,
            nanos: 123456789,
        };

        let rfc3339 = to_rfc3339(&original).unwrap();
        let converted = from_rfc3339(&rfc3339).unwrap();

        assert_eq!(original.seconds, converted.seconds);
        assert_eq!(original.nanos, converted.nanos);
    }
}
