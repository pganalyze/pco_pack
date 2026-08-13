use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(timestamp = ts)]
struct DateTimeRecord {
    ts: chrono::DateTime<chrono::Utc>,
}

#[test]
fn datetime_utc_roundtrip() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1700000000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(-1000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let result = DateTimeRecord::deserialize(&bytes).unwrap();
    let mut sorted_data = data.clone();
    sorted_data.sort_by_key(|r| r.ts);
    assert_eq!(result, sorted_data);
}

#[test]
fn datetime_utc_filter_exact() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(2000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(3000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": 1_000_000_000});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].ts, data[1].ts);
    assert_eq!(result[1].ts, data[3].ts);
}

#[test]
fn datetime_utc_filter_range() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(2000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(3000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(5000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": {"start": 1_000_000_000i64, "end": 3_000_000_000i64}});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].ts, data[1].ts);
    assert_eq!(result[1].ts, data[2].ts);
    assert_eq!(result[2].ts, data[3].ts);
}

#[test]
fn datetime_utc_filter_inclusion() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(2000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(3000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(5000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": [1_000_000_000i64, 5_000_000_000i64]});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].ts, data[1].ts);
    assert_eq!(result[1].ts, data[4].ts);
}

#[test]
fn datetime_utc_filter_no_match() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": 999_999_999});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn datetime_utc_filter_exact_rfc3339() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(2000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": "1970-01-01T00:16:40Z"});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].ts, data[1].ts);
}

#[test]
fn datetime_utc_filter_range_rfc3339() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(2000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(3000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(5000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": {
        "start": "1970-01-01T00:16:40Z",
        "end": "1970-01-01T00:50:00Z"
    }});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 3);
}

#[test]
fn datetime_utc_filter_inclusion_rfc3339() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(5000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": ["1970-01-01T00:16:40Z", "1970-01-01T01:23:20Z"]});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn datetime_utc_filter_rfc3339_with_timezone() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"ts": "1970-01-01T05:16:40+05:00"});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn datetime_utc_filter_rfc3339_mixed_range() {
    let data = vec![
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(1000, 0).unwrap() },
        DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::from_timestamp(2000, 0).unwrap() },
    ];
    let bytes = DateTimeRecord::serialize(data.clone()).unwrap();
    // Start as RFC3339 string, end as integer
    let query = serde_json::json!({"ts": {
        "start": "1970-01-01T00:16:40Z",
        "end": 2_000_000_000i64
    }});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn datetime_utc_filter_invalid_rfc3339() {
    let data = vec![DateTimeRecord { ts: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH }];
    let bytes = DateTimeRecord::serialize(data).unwrap();
    let query = serde_json::json!({"ts": "not-a-datetime"});
    let result = DateTimeRecord::filter_bytes(&bytes, query, &[]);
    assert!(result.is_err());
}
