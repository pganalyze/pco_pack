use crate as pco_pack;
use crate::PcoPack;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = ts, time_round = chrono::Duration::seconds(60))]
struct DateTimeMinuteRound {
    ts: DateTime<Utc>,
    value: i64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = ts, time_round = chrono::Duration::seconds(3600))]
struct DateTimeHourRound {
    ts: DateTime<Utc>,
    value: i64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct DateTimePayloadRound {
    id: i64,
    ts: DateTime<Utc>,
}

#[test]
fn datetime_minute_round_roundtrip() {
    let data = vec![DateTimeMinuteRound { ts: Utc::now(), value: 1 }];

    let bytes = DateTimeMinuteRound::serialize(data.clone()).unwrap();
    let result = DateTimeMinuteRound::deserialize(&bytes).unwrap();
    assert_eq!(result.len(), data.len());
}

#[test]
fn datetime_minute_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();

    let data = vec![
        DateTimeMinuteRound { ts: base, value: 1 },
        DateTimeMinuteRound { ts: base + Duration::seconds(30), value: 2 },
        DateTimeMinuteRound { ts: base + Duration::seconds(45), value: 3 },
        DateTimeMinuteRound { ts: base + Duration::seconds(59), value: 4 },
    ];

    let bytes = DateTimeMinuteRound::serialize(data.clone()).unwrap();
    let result = DateTimeMinuteRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 4);
    assert_eq!(result[0].ts, rounded_base);
    assert_eq!(result[1].ts, rounded_next);
    assert_eq!(result[2].ts, rounded_next);
    assert_eq!(result[3].ts, rounded_next);
}

#[test]
fn datetime_hour_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999200, 0).unwrap(); // 2023-11-14T22:00:00Z
    let rounded_12 = DateTime::<Utc>::from_timestamp(1700042400, 0).unwrap(); // 2023-11-15T10:00:00Z
    let rounded_12h30 = DateTime::<Utc>::from_timestamp(1700046000, 0).unwrap(); // 2023-11-15T11:00:00Z

    let data = vec![
        DateTimeHourRound { ts: base, value: 1 },
        DateTimeHourRound { ts: base + Duration::hours(12), value: 2 },
        DateTimeHourRound { ts: base + Duration::hours(12) + Duration::minutes(30), value: 3 },
    ];

    let bytes = DateTimeHourRound::serialize(data.clone()).unwrap();
    let result = DateTimeHourRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].ts, rounded_base);
    assert_eq!(result[1].ts, rounded_12);
    assert_eq!(result[2].ts, rounded_12h30);
}

#[test]
fn datetime_payload_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let data = vec![
        DateTimePayloadRound { id: 1, ts: base },
        DateTimePayloadRound { id: 2, ts: base + Duration::seconds(30) },
        DateTimePayloadRound { id: 3, ts: base + Duration::seconds(45) },
    ];

    let bytes = DateTimePayloadRound::serialize(data.clone()).unwrap();
    let result = DateTimePayloadRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].ts, rounded_base);
    assert_eq!(result[1].ts, rounded_next);
    assert_eq!(result[2].ts, rounded_next);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = ts)]
struct DateTimeNoRound {
    ts: DateTime<Utc>,
    value: i64,
}

#[test]
fn datetime_no_round_preserves_precision() {
    let data = vec![
        DateTimeNoRound { ts: DateTime::<Utc>::from_timestamp(1700000000, 123000).unwrap(), value: 1 },
        DateTimeNoRound { ts: DateTime::<Utc>::from_timestamp(1700000001, 654000).unwrap(), value: 2 },
    ];

    let bytes = DateTimeNoRound::serialize(data.clone()).unwrap();
    let result = DateTimeNoRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].ts, data[0].ts);
    assert_eq!(result[1].ts, data[1].ts);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct InnerWithTime {
    ts: DateTime<Utc>,
    value: i64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct OuterNoTimeRound {
    id: i64,
    inner: InnerWithTime,
}

#[test]
fn nested_struct_time_round_propagates() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let data = vec![
        OuterNoTimeRound { id: 1, inner: InnerWithTime { ts: base, value: 1 } },
        OuterNoTimeRound { id: 2, inner: InnerWithTime { ts: base + Duration::seconds(30), value: 2 } },
    ];

    let bytes = OuterNoTimeRound::serialize(data.clone()).unwrap();
    let result = OuterNoTimeRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].inner.ts, rounded_base);
    assert_eq!(result[1].inner.ts, rounded_next);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [device_id], timestamp = ts, time_round = chrono::Duration::seconds(60))]
struct GroupedDateTimeRound {
    device_id: i64,
    ts: DateTime<Utc>,
    value: i64,
}

#[test]
fn grouped_datetime_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let data = vec![
        GroupedDateTimeRound { device_id: 1, ts: base, value: 1 },
        GroupedDateTimeRound { device_id: 1, ts: base + Duration::seconds(30), value: 2 },
        GroupedDateTimeRound { device_id: 2, ts: base + Duration::seconds(45), value: 3 },
    ];

    let bytes = GroupedDateTimeRound::serialize(data.clone()).unwrap();
    let result = GroupedDateTimeRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    // Device 1: base -> 22:13:00Z, 30s -> 22:14:00Z
    let dev1: Vec<_> = result.iter().filter(|r| r.device_id == 1).collect();
    assert_eq!(dev1[0].ts, rounded_base);
    assert_eq!(dev1[1].ts, rounded_next);
    // Device 2: 45s -> 22:14:00Z
    let dev2: Vec<_> = result.iter().filter(|r| r.device_id == 2).collect();
    assert_eq!(dev2[0].ts, rounded_next);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct VecDateTimeRound {
    id: i64,
    timestamps: Vec<DateTime<Utc>>,
}

#[test]
fn vec_datetime_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let data = vec![VecDateTimeRound {
        id: 1,
        timestamps: vec![base, base + Duration::seconds(30), base + Duration::seconds(45)],
    }];

    let bytes = VecDateTimeRound::serialize(data.clone()).unwrap();
    let result = VecDateTimeRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timestamps.len(), 3);
    assert_eq!(result[0].timestamps[0], rounded_base);
    assert_eq!(result[0].timestamps[1], rounded_next);
    assert_eq!(result[0].timestamps[2], rounded_next);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct OptDateTimeRound {
    id: i64,
    ts: Option<DateTime<Utc>>,
}

#[test]
fn opt_datetime_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let data = vec![
        OptDateTimeRound { id: 1, ts: Some(base) },
        OptDateTimeRound { id: 2, ts: None },
        OptDateTimeRound { id: 3, ts: Some(base + Duration::seconds(30)) },
    ];

    let bytes = OptDateTimeRound::serialize(data.clone()).unwrap();
    let result = OptDateTimeRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].ts, Some(rounded_base));
    assert!(result[1].ts.is_none());
    assert_eq!(result[2].ts, Some(rounded_next));
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct MapDateTimeRound {
    id: i64,
    events: std::collections::HashMap<String, DateTime<Utc>>,
}

#[test]
fn map_datetime_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let mut events = std::collections::HashMap::new();
    events.insert("start".to_string(), base);
    events.insert("end".to_string(), base + Duration::seconds(45));

    let data = vec![MapDateTimeRound { id: 1, events }];

    let bytes = MapDateTimeRound::serialize(data.clone()).unwrap();
    let result = MapDateTimeRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].events["start"], rounded_base);
    assert_eq!(result[0].events["end"], rounded_next);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct TupleDateTimeRound {
    id: i64,
    range: (DateTime<Utc>, DateTime<Utc>),
}

#[test]
fn tuple_datetime_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1699999980, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let data = vec![TupleDateTimeRound { id: 1, range: (base, base + Duration::seconds(45)) }];

    let bytes = TupleDateTimeRound::serialize(data.clone()).unwrap();
    let result = TupleDateTimeRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].range.0, rounded_base);
    assert_eq!(result[0].range.1, rounded_next);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct JsonValueWithTimestamp {
    id: i64,
    data: serde_json::Value,
}

#[test]
fn json_value_timestamp_no_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let original_micros = base.timestamp_micros();
    let data = vec![JsonValueWithTimestamp { id: 1, data: serde_json::json!({ "ts": original_micros }) }];

    let bytes = JsonValueWithTimestamp::serialize(data.clone()).unwrap();
    let result = JsonValueWithTimestamp::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].data["ts"], serde_json::json!(original_micros));
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(86400))]
struct DailyRound {
    id: i64,
    ts: DateTime<Utc>,
}

#[test]
fn daily_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_base = DateTime::<Utc>::from_timestamp(1700006400, 0).unwrap(); // 2023-11-15T00:00:00Z
    let base_plus_12h = DateTime::<Utc>::from_timestamp(1700043200, 0).unwrap();
    let rounded_12h = DateTime::<Utc>::from_timestamp(1700006400, 0).unwrap(); // 2023-11-15T00:00:00Z
    let data = vec![DailyRound { id: 1, ts: base }, DailyRound { id: 2, ts: base_plus_12h }];

    let bytes = DailyRound::serialize(data.clone()).unwrap();
    let result = DailyRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].ts, rounded_base);
    assert_eq!(result[1].ts, rounded_12h);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
enum EventWithTime {
    #[default]
    None,
    Started(DateTime<Utc>),
    Finished(DateTime<Utc>),
}

#[test]
fn enum_datetime_round_applied() {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    let rounded_next = DateTime::<Utc>::from_timestamp(1700000040, 0).unwrap();
    let data = vec![
        EventWithTime::None,
        EventWithTime::Started(base + Duration::seconds(30)),
        EventWithTime::Finished(base + Duration::seconds(45)),
    ];

    let bytes = EventWithTime::serialize(data.clone()).unwrap();
    let result = EventWithTime::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0], EventWithTime::None);
    assert_eq!(result[1], EventWithTime::Started(rounded_next));
    assert_eq!(result[2], EventWithTime::Finished(rounded_next));
}
