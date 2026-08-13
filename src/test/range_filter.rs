use crate as pco_pack;
use crate::PcoPack;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = collected_at, index = [database_id])]
struct QueryStat {
    database_id: i64,
    collected_at: i64,
    fingerprint: i64,
    calls: i64,
    rows: i64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = collected_at)]
struct TimeSeries {
    collected_at: i64,
    value: i64,
    label: String,
}

#[test]
fn range_sorts_and_adds_metadata() {
    let data = vec![
        TimeSeries { collected_at: 300, value: 3, label: "c".into() },
        TimeSeries { collected_at: 100, value: 1, label: "a".into() },
        TimeSeries { collected_at: 200, value: 2, label: "b".into() },
    ];

    let bytes = TimeSeries::serialize(data.clone()).unwrap();
    let result = TimeSeries::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result[0].collected_at, 100);
    assert_eq!(result[1].collected_at, 200);
    assert_eq!(result[2].collected_at, 300);
}

#[test]
fn range_filter_basic() {
    let now = Utc::now().timestamp_micros();
    let data = vec![
        QueryStat { database_id: 1, collected_at: now, fingerprint: 100, calls: 50, rows: 1000 },
        QueryStat { database_id: 1, collected_at: now, fingerprint: 101, calls: 55, rows: 1100 },
        QueryStat { database_id: 2, collected_at: now, fingerprint: 200, calls: 30, rows: 500 },
    ];

    let buf = QueryStat::serialize(data).unwrap();
    let result =
        QueryStat::filter_bytes(&buf, serde_json::json!({"collected_at": {"start": i64::MIN, "end": i64::MAX}}), &[])
            .unwrap();
    assert_eq!(result.len(), 3);
}

#[test]
fn range_filter_skips_groups() {
    #[derive(Debug, Clone, PcoPack)]
    #[pco_pack(timestamp = collected_at)]
    struct TimeSeriesData {
        collected_at: DateTime<Utc>,
        value: f64,
    }

    let now = Utc::now();
    let data = vec![
        TimeSeriesData { collected_at: now - Duration::hours(10), value: 1.0 },
        TimeSeriesData { collected_at: now - Duration::hours(5), value: 2.0 },
        TimeSeriesData { collected_at: now - Duration::hours(2), value: 3.0 },
        TimeSeriesData { collected_at: now - Duration::hours(1), value: 4.0 },
    ];

    let buf = TimeSeriesData::serialize(data.clone()).unwrap();

    let start = (now - Duration::hours(3)).timestamp_micros();
    let end = now.timestamp_micros();
    let result =
        TimeSeriesData::filter_bytes(&buf, serde_json::json!({"collected_at": {"start": start, "end": end}}), &[])
            .unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn range_filter_prunes_groups() {
    #[derive(Debug, Clone, PcoPack)]
    #[pco_pack(timestamp = collected_at, index = [device_id])]
    struct GroupedTimeSeries {
        device_id: i64,
        collected_at: i64,
        value: f64,
    }

    let data = vec![
        GroupedTimeSeries { device_id: 1, collected_at: 100, value: 1.0 },
        GroupedTimeSeries { device_id: 1, collected_at: 200, value: 2.0 },
        GroupedTimeSeries { device_id: 2, collected_at: 500, value: 3.0 },
        GroupedTimeSeries { device_id: 2, collected_at: 600, value: 4.0 },
        GroupedTimeSeries { device_id: 1, collected_at: 700, value: 5.0 },
    ];

    let buf = GroupedTimeSeries::serialize(data).unwrap();

    let filter = serde_json::json!({"collected_at": {"start": 150, "end": 650}});
    let mut result = GroupedTimeSeries::filter_bytes(&buf, filter.clone(), &[]).unwrap();
    result.sort_by_key(|r| r.device_id);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].collected_at, 200);
    assert_eq!(result[1].collected_at, 500);
    assert_eq!(result[2].collected_at, 600);
}

#[test]
fn range_with_float_round() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(timestamp = timestamp, float_round = 2)]
    struct TimeSeriesRounded {
        timestamp: i64,
        value: f64,
    }

    let data = vec![
        TimeSeriesRounded { timestamp: 100, value: 1.234567 },
        TimeSeriesRounded { timestamp: 200, value: 2.345678 },
        TimeSeriesRounded { timestamp: 300, value: 3.456789 },
    ];

    let bytes = TimeSeriesRounded::serialize(data.clone()).unwrap();
    let result = TimeSeriesRounded::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result[0].timestamp, 100);
    assert!((result[0].value - 1.23).abs() < 1e-10);
    assert_eq!(result[1].timestamp, 200);
    assert!((result[1].value - 2.35).abs() < 1e-10);
    assert_eq!(result[2].timestamp, 300);
    assert!((result[2].value - 3.46).abs() < 1e-10);
}

#[test]
fn range_filter_start_equals_end_i64() {
    #[derive(Debug, Clone, PcoPack)]
    struct SingleValRow {
        id: i64,
        value: i32,
    }

    let data = vec![
        SingleValRow { id: 1, value: 10 },
        SingleValRow { id: 2, value: 50 },
        SingleValRow { id: 3, value: 50 },
        SingleValRow { id: 4, value: 90 },
    ];

    let buf = SingleValRow::serialize(data.clone()).unwrap();

    let result = SingleValRow::filter_bytes(&buf, serde_json::json!({"value": {"start": 50, "end": 50}}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 2);
    assert_eq!(result[1].id, 3);
}

#[test]
fn range_filter_start_equals_end_f64() {
    #[derive(Debug, Clone, PcoPack)]
    struct FloatRow {
        id: i64,
        score: f32,
    }

    let data = vec![FloatRow { id: 1, score: 90.0 }, FloatRow { id: 2, score: 85.5 }, FloatRow { id: 3, score: 85.5 }];

    let buf = FloatRow::serialize(data.clone()).unwrap();

    let result = FloatRow::filter_bytes(&buf, serde_json::json!({"score": {"start": 85.5, "end": 85.5}}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 2);
    assert_eq!(result[1].id, 3);
}

#[test]
fn range_filter_start_equals_end_no_match() {
    #[derive(Debug, Clone, PcoPack)]
    struct Row {
        id: i64,
        value: i32,
    }

    let data = vec![Row { id: 1, value: 10 }, Row { id: 2, value: 30 }];

    let buf = Row::serialize(data).unwrap();
    let result = Row::filter_bytes(&buf, serde_json::json!({"value": {"start": 50, "end": 50}}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn range_filter_start_greater_than_end_returns_nothing() {
    #[derive(Debug, Clone, PcoPack)]
    struct Row {
        id: i64,
        value: i32,
    }

    let data = vec![Row { id: 1, value: 10 }, Row { id: 2, value: 50 }, Row { id: 3, value: 90 }];

    let buf = Row::serialize(data).unwrap();

    let result = Row::filter_bytes(&buf, serde_json::json!({"value": {"start": 80, "end": 20}}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn range_filter_start_greater_than_end_float_returns_nothing() {
    #[derive(Debug, Clone, PcoPack)]
    struct Row {
        id: i64,
        score: f32,
    }

    let data = vec![Row { id: 1, score: 90.0 }, Row { id: 2, score: 50.0 }];

    let buf = Row::serialize(data).unwrap();
    let result = Row::filter_bytes(&buf, serde_json::json!({"score": {"start": 80.0, "end": 20.0}}), &[]).unwrap();
    assert!(result.is_empty());
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = ts)]
struct TimestampedRow {
    ts: i64,
    value: i32,
}

#[test]
fn timestamp_chunk_metadata_single_chunk() {
    let data = vec![
        TimestampedRow { ts: 100, value: 1 },
        TimestampedRow { ts: 200, value: 2 },
        TimestampedRow { ts: 300, value: 3 },
    ];

    let chunks = TimestampedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].start_at.timestamp_micros(), 100);
    assert_eq!(chunks[0].end_at.timestamp_micros(), 300);
}

#[test]
fn timestamp_chunk_metadata_unsorted_input() {
    let data = vec![
        TimestampedRow { ts: 500, value: 1 },
        TimestampedRow { ts: 100, value: 2 },
        TimestampedRow { ts: 300, value: 3 },
    ];

    let chunks = TimestampedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].start_at.timestamp_micros(), 100);
    assert_eq!(chunks[0].end_at.timestamp_micros(), 500);
}

#[test]
fn timestamp_chunk_metadata_single_row() {
    let data = vec![TimestampedRow { ts: 42, value: 1 }];

    let chunks = TimestampedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].start_at.timestamp_micros(), 42);
    assert_eq!(chunks[0].end_at.timestamp_micros(), 42);
}

#[test]
fn timestamp_chunk_metadata_multiple_chunks() {
    let n = TimestampedRow::CHUNK_SIZE + 5;
    let data: Vec<TimestampedRow> = (0..n).map(|i| TimestampedRow { ts: i as i64, value: i as i32 }).collect();

    let chunks = TimestampedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 2);

    assert_eq!(chunks[0].start_at.timestamp_micros(), 0);
    assert_eq!(chunks[0].end_at.timestamp_micros(), (TimestampedRow::CHUNK_SIZE - 1) as i64);
    assert_eq!(chunks[1].start_at.timestamp_micros(), TimestampedRow::CHUNK_SIZE as i64);
    assert_eq!(chunks[1].end_at.timestamp_micros(), (n - 1) as i64);
}

#[test]
fn timestamp_chunk_metadata_with_index() {
    // With index + timestamp: each group gets its own chunk(s); verify per-group bounds.
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(timestamp = ts, index = [group_id])]
    struct GroupedRow {
        group_id: i64,
        ts: i64,
        value: i32,
    }

    let data = vec![
        GroupedRow { group_id: 1, ts: 100, value: 1 },
        GroupedRow { group_id: 2, ts: 500, value: 2 },
        GroupedRow { group_id: 1, ts: 300, value: 3 },
        GroupedRow { group_id: 2, ts: 600, value: 4 },
        GroupedRow { group_id: 1, ts: 200, value: 5 },
    ];

    let mut chunks = GroupedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 2);
    chunks.sort_by_key(|c| c.group_id);
    assert_eq!(chunks[0].group_id, 1);
    assert_eq!(chunks[0].start_at.timestamp_micros(), 100);
    assert_eq!(chunks[0].end_at.timestamp_micros(), 300);
    assert_eq!(chunks[1].group_id, 2);
    assert_eq!(chunks[1].start_at.timestamp_micros(), 500);
    assert_eq!(chunks[1].end_at.timestamp_micros(), 600);
}

#[test]
fn timestamp_chunk_metadata_survives_roundtrip() {
    let data = vec![
        TimestampedRow { ts: 1000, value: 1 },
        TimestampedRow { ts: 2000, value: 2 },
        TimestampedRow { ts: 3000, value: 3 },
    ];

    let chunks = TimestampedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 1);

    let bytes = TimestampedRow::to_bytes(&chunks).unwrap();
    let restored_chunks = TimestampedRow::from_bytes(&bytes).unwrap();
    assert_eq!(restored_chunks.len(), 1);
    assert_eq!(restored_chunks[0].start_at, chunks[0].start_at);
    assert_eq!(restored_chunks[0].end_at, chunks[0].end_at);
}

#[test]
fn timestamp_chunk_metadata_negative_timestamps() {
    let data = vec![
        TimestampedRow { ts: -500, value: 1 },
        TimestampedRow { ts: -200, value: 2 },
        TimestampedRow { ts: 100, value: 3 },
    ];

    let chunks = TimestampedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].start_at.timestamp_micros(), -500);
    assert_eq!(chunks[0].end_at.timestamp_micros(), 100);
}

#[test]
fn timestamp_chunk_metadata_all_same_timestamp() {
    let data = vec![
        TimestampedRow { ts: 42, value: 1 },
        TimestampedRow { ts: 42, value: 2 },
        TimestampedRow { ts: 42, value: 3 },
    ];

    let chunks = TimestampedRow::write(data).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].start_at.timestamp_micros(), 42);
    assert_eq!(chunks[0].end_at.timestamp_micros(), 42);
}
