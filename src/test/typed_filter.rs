use crate as pco_pack;
use crate::{BoolFilter, DateTimeFilter, F64Filter, I64Filter, PcoFilter, PcoPack, StringFilter, UuidFilter};
use chrono::{DateTime, Duration, Utc};

#[derive(PcoPack, Debug, PartialEq)]
#[pco_pack(index = [device_id], timestamp = collected_at)]
struct Sensor {
    device_id: i64,
    collected_at: DateTime<Utc>,
    temperature: f64,
}

#[derive(PcoPack, Debug, PartialEq)]
#[pco_pack(index = [db_id, granularity])]
struct QueryStat {
    db_id: i64,
    granularity: i32,
    calls: i64,
}

// Matches query_stats.rs from pco_pack_derive - timestamp + multi-index fields.
#[derive(PcoPack, Debug, PartialEq)]
#[pco_pack(timestamp = collected_at, index = [database_id, granularity])]
struct QueryStatWithTimestamp {
    database_id: i64,
    granularity: i32,
    collected_at: DateTime<Utc>,
    fingerprint: i64,
}
type QueryStatWithTimestampFilter = <QueryStatWithTimestamp as PcoPack>::Filter;

#[derive(PcoPack, Debug, PartialEq)]
#[pco_pack(timestamp = ts)]
struct Event {
    ts: DateTime<Utc>,
    name: String,
    value: f64,
}

#[derive(PcoPack, Debug, PartialEq)]
#[pco_pack(index = [name])]
struct NamedItem {
    name: String,
    count: i32,
}

#[derive(PcoPack, Debug, PartialEq)]
#[pco_pack(index = [id], timestamp = created_at)]
struct UuidRecord {
    id: uuid::Uuid,
    created_at: DateTime<Utc>,
    label: String,
}

type SensorFilter = <Sensor as PcoPack>::Filter;
type QueryStatFilter = <QueryStat as PcoPack>::Filter;
type EventFilter = <Event as PcoPack>::Filter;
type NamedItemFilter = <NamedItem as PcoPack>::Filter;
type UuidRecordFilter = <UuidRecord as PcoPack>::Filter;

#[test]
fn filter_new_with_index_and_timestamp() {
    let now = Utc::now();
    let range = (now - Duration::minutes(10))..=now;

    let filter = SensorFilter::new(I64Filter::Equal(1), range.clone());

    assert_eq!(filter.device_id, Some(I64Filter::Equal(1)));
    assert_eq!(filter.collected_at, Some(DateTimeFilter::Range { start: *range.start(), end: *range.end() }));
}

#[test]
fn filter_new_with_into_impls() {
    let now = Utc::now();
    // Use Into impls for ergonomic construction
    let filter = SensorFilter::new(1i64, (now - Duration::minutes(5))..=now);

    assert_eq!(filter.device_id, Some(I64Filter::Equal(1)));
}

#[test]
fn filter_index_mut_for_arbitrary_fields() {
    let now = Utc::now();
    let mut filter = SensorFilter::new(1i64, (now - Duration::minutes(5))..=now);

    // Set arbitrary fields via IndexMut
    filter["temperature"] = serde_json::json!({ "start": 20.0, "end": 30.0 });
    filter["nested.field"] = serde_json::json!("value");

    assert_eq!(filter.get("temperature"), Some(&serde_json::json!({ "start": 20.0, "end": 30.0 })));
    assert_eq!(filter.get("nested.field"), Some(&serde_json::json!("value")));
}

#[test]
fn filter_index_returns_null_for_missing() {
    let now = Utc::now();
    let filter = SensorFilter::new(1i64, (now - Duration::minutes(5))..=now);

    assert_eq!(filter["nonexistent"], serde_json::Value::Null);
}

#[test]
fn filter_set_method() {
    let now = Utc::now();
    let mut filter = SensorFilter::new(1i64, (now - Duration::minutes(5))..=now);

    filter.set("temperature", serde_json::json!(25.0));

    assert_eq!(filter.get("temperature"), Some(&serde_json::json!(25.0)));
}

#[test]
fn filter_serialize_to_json() {
    let now = Utc::now();
    let mut filter = SensorFilter::new(I64Filter::Equal(1), (now - Duration::minutes(10))..=now);
    filter["temperature"] = serde_json::json!({ "start": 20.0, "end": 30.0 });

    let json: serde_json::Value = filter.try_into().unwrap();

    assert_eq!(json["device_id"], serde_json::json!(1));
    assert!(json["collected_at"]["start"].is_string()); // DateTime serializes as RFC3339 string
    assert!(json["collected_at"]["end"].is_string());
    assert_eq!(json["temperature"], serde_json::json!({ "start": 20.0, "end": 30.0 }));
}

#[test]
fn filter_deserialize_from_json() {
    let json = serde_json::json!({
        "device_id": 1,
        "collected_at": {
            "start": "2026-01-01T00:00:00Z",
            "end": "2026-01-01T01:00:00Z"
        },
        "temperature": { "start": 20.0, "end": 30.0 }
    });

    let filter: SensorFilter = json.try_into().unwrap();

    assert_eq!(filter.device_id, Some(I64Filter::Equal(1)));
    if let Some(DateTimeFilter::Range { start, end }) = &filter.collected_at {
        assert_eq!(start.to_rfc3339(), "2026-01-01T00:00:00+00:00");
        assert_eq!(end.to_rfc3339(), "2026-01-01T01:00:00+00:00");
    } else {
        panic!("Expected DateTimeFilter::Range");
    }
    // temperature is now a typed field; deserialize into F64Filter
    if let Some(F64Filter::Range { start, end }) = &filter.temperature {
        assert_eq!(*start, 20.0);
        assert_eq!(*end, 30.0);
    } else {
        panic!("Expected F64Filter::Range");
    }
}

#[test]
fn filter_roundtrip_json() {
    let now = Utc::now();
    let mut original = SensorFilter::new(I64Filter::Equal(1), (now - Duration::minutes(10))..=now);
    original.temperature = Some((20.0..=30.0).into());

    let json: serde_json::Value = original.try_into().unwrap();
    let restored: SensorFilter = json.try_into().unwrap();

    assert_eq!(restored.device_id, Some(I64Filter::Equal(1)));
    if let Some(F64Filter::Range { start, end }) = &restored.temperature {
        assert_eq!(*start, 20.0);
        assert_eq!(*end, 30.0);
    } else {
        panic!("Expected F64Filter::Range");
    }
}

#[test]
fn multi_index_filter_new() {
    let filter = QueryStatFilter::new(I64Filter::Equal(1), I64Filter::Equal(5));

    assert_eq!(filter.db_id, Some(I64Filter::Equal(1)));
    assert_eq!(filter.granularity, Some(I64Filter::Equal(5)));
}

#[test]
fn multi_index_filter_serialize() {
    let filter = QueryStatFilter::new(1i64, 5i32);
    let json: serde_json::Value = filter.try_into().unwrap();

    assert_eq!(json["db_id"], 1);
    assert_eq!(json["granularity"], 5);
}

#[test]
fn index_filter_range_from_range_inclusive() {
    let f: I64Filter = (5..=10).into();
    match f {
        I64Filter::Range { start, end } => {
            assert_eq!(start, 5);
            assert_eq!(end, 10);
        }
        _ => panic!("Expected Range"),
    }
}

#[test]
fn index_filter_range_from_other_int_types() {
    let f: I64Filter = (5i32..=10i32).into();
    match f {
        I64Filter::Range { start, end } => assert_eq!((start, end), (5, 10)),
        _ => panic!("Expected Range"),
    }

    let f: I64Filter = (1u8..=3u8).into();
    match f {
        I64Filter::Range { start, end } => assert_eq!((start, end), (1, 3)),
        _ => panic!("Expected Range"),
    }
}

#[test]
fn index_filter_range_integration() {
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now, temperature: 20.0 },
        Sensor { device_id: 2, collected_at: now, temperature: 25.0 },
        Sensor { device_id: 3, collected_at: now, temperature: 30.0 },
        Sensor { device_id: 4, collected_at: now, temperature: 35.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    let mut filter = SensorFilter::new(2i64..=3, now..=now);
    filter.temperature = Some(F64Filter::Equal(25.0));

    let result = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].device_id, 2);
}

#[test]
fn index_filter_range_multi_index() {
    let data = vec![
        QueryStat { db_id: 1, granularity: 5, calls: 100 },
        QueryStat { db_id: 1, granularity: 10, calls: 200 },
        QueryStat { db_id: 2, granularity: 5, calls: 300 },
        QueryStat { db_id: 2, granularity: 15, calls: 400 },
    ];

    let bytes = QueryStat::serialize(data).unwrap();

    // Range filter on first index field (db_id)
    let mut filter = QueryStatFilter::new(1i64..=1, I64Filter::Equal(5));
    filter.calls = Some(I64Filter::Equal(100));

    let result = QueryStat::filter_bytes(&bytes, filter, &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].db_id, 1);
    assert_eq!(result[0].granularity, 5);
}

#[test]
fn index_filter_range_no_match() {
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now, temperature: 20.0 },
        Sensor { device_id: 5, collected_at: now, temperature: 30.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    let filter = SensorFilter::new(10i64..=20, now..=now);
    let result = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();

    assert!(result.is_empty());
}

#[test]
fn timestamp_only_filter_new() {
    let now = Utc::now();
    let filter = EventFilter::new((now - Duration::hours(1))..=now);

    assert!(matches!(&filter.ts, Some(DateTimeFilter::Range { .. })));
}

#[test]
fn timestamp_only_filter_serialize() {
    let now = Utc::now();
    let filter = EventFilter::new((now - Duration::hours(1))..=now);
    let json: serde_json::Value = filter.try_into().unwrap();

    assert!(json["ts"]["start"].is_string());
    assert!(json["ts"]["end"].is_string());
}

#[test]
fn i64_filter_equal() {
    let f = I64Filter::Equal(42);
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, 42);
}

#[test]
fn i64_filter_inclusion() {
    let f = I64Filter::Inclusion(vec![1, 2, 3]);
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, serde_json::json!([1, 2, 3]));
}

#[test]
fn i64_filter_range() {
    let f = I64Filter::Range { start: 0, end: 10 };
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, serde_json::json!({ "start": 0, "end": 10 }));
}

#[test]
fn i64_filter_from_int() {
    let f: I64Filter = 42i64.into();
    match f {
        I64Filter::Equal(v) => assert_eq!(v, 42),
        _ => panic!("Expected Equal"),
    }
}

#[test]
fn i64_filter_from_range_inclusive() {
    let f: I64Filter = (0..=10).into();
    match f {
        I64Filter::Range { start, end } => {
            assert_eq!(start, 0);
            assert_eq!(end, 10);
        }
        _ => panic!("Expected Range"),
    }
}

#[test]
fn i64_filter_from_slice() {
    let f: I64Filter = [1i64, 2, 3].into();
    match f {
        I64Filter::Inclusion(v) => assert_eq!(v, vec![1, 2, 3]),
        _ => panic!("Expected Inclusion"),
    }
}

#[test]
fn f64_filter_equal() {
    let f = F64Filter::Equal(3.14);
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, 3.14);
}

#[test]
fn f64_filter_range() {
    let f = F64Filter::Range { start: 0.0, end: 1.0 };
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, serde_json::json!({ "start": 0.0, "end": 1.0 }));
}

#[test]
fn string_filter_equal() {
    let f = StringFilter::Equal("hello".into());
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, "hello");
}

#[test]
fn string_filter_inclusion() {
    let f = StringFilter::Inclusion(vec!["a".into(), "b".into()]);
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, serde_json::json!(["a", "b"]));
}

#[test]
fn bool_filter_equal() {
    let f = BoolFilter::Equal(true);
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(json, true);
}

#[test]
fn datetime_filter_range() {
    let now = Utc::now();
    let f = DateTimeFilter::Range { start: now - Duration::hours(1), end: now };
    let json: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert!(json["start"].is_string());
    assert!(json["end"].is_string());
}

#[test]
fn datetime_filter_from_range_inclusive() {
    let start = Utc::now();
    let end = start + Duration::hours(1);
    let range: std::ops::RangeInclusive<DateTime<Utc>> = start..=end;
    let f: DateTimeFilter = range.into();

    match f {
        DateTimeFilter::Range { start: s, end: e } => {
            assert_eq!(s.to_rfc3339(), start.to_rfc3339());
            assert_eq!(e.to_rfc3339(), end.to_rfc3339());
        }
        _ => panic!("Expected Range"),
    }
}

#[test]
fn typed_filter_used_in_filter_bytes() {
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 23.0 },
        Sensor { device_id: 2, collected_at: now, temperature: 42.0 },
        Sensor { device_id: 1, collected_at: now - Duration::minutes(15), temperature: 25.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    let mut filter = SensorFilter::new(I64Filter::Equal(1), (now - Duration::minutes(10))..=now);
    filter["temperature"] = serde_json::json!({ "start": 20.0, "end": 30.0 });

    let results = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].device_id, 1);
    assert!((results[0].temperature - 23.0).abs() < f64::EPSILON);
}

#[test]
fn typed_filter_inclusion_used_in_filter_bytes() {
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now, temperature: 23.0 },
        Sensor { device_id: 2, collected_at: now, temperature: 42.0 },
        Sensor { device_id: 3, collected_at: now, temperature: 55.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    let filter =
        SensorFilter::new(I64Filter::Inclusion(vec![1, 3]), (now - Duration::hours(1))..=now + Duration::hours(1));

    let results = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn typed_filter_with_multi_index() {
    let data = vec![
        QueryStat { db_id: 1, granularity: 60, calls: 100 },
        QueryStat { db_id: 1, granularity: 300, calls: 50 },
        QueryStat { db_id: 2, granularity: 60, calls: 75 },
    ];

    let bytes = QueryStat::serialize(data).unwrap();

    let filter = QueryStatFilter::new(1i64, I64Filter::Equal(60));

    let results = QueryStat::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].calls, 100);
}

#[test]
fn typed_filter_empty_others_serializes_minimal() {
    let now = Utc::now();
    let filter = SensorFilter::new(1i64, (now - Duration::minutes(5))..=now);
    let json: serde_json::Value = filter.try_into().unwrap();

    assert_eq!(json.as_object().unwrap().len(), 2);
}

#[test]
fn string_index_filter_new() {
    let filter = NamedItemFilter::new("alice".to_string());
    assert_eq!(filter.name, Some(StringFilter::Equal("alice".into())));
}

#[test]
fn string_index_filter_from_str() {
    let filter = NamedItemFilter::new("bob");
    assert_eq!(filter.name, Some(StringFilter::Equal("bob".into())));
}

#[test]
fn string_index_filter_inclusion() {
    let filter = NamedItemFilter::new(&["alice", "bob"][..]);
    match &filter.name {
        Some(StringFilter::Inclusion(v)) => {
            assert_eq!(v.len(), 2);
            assert_eq!(v[0], "alice");
            assert_eq!(v[1], "bob");
        }
        _ => panic!("Expected Inclusion"),
    }
}

#[test]
fn string_index_filter_integration() {
    let data = vec![
        NamedItem { name: "alice".into(), count: 10 },
        NamedItem { name: "bob".into(), count: 20 },
        NamedItem { name: "charlie".into(), count: 30 },
    ];

    let bytes = NamedItem::serialize(data).unwrap();

    // Exact match via From<&str>
    let filter = NamedItemFilter::new("alice");
    let results = NamedItem::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "alice");

    // Inclusion via From<[&str; N]>
    let filter = NamedItemFilter::new(["bob", "charlie"]);
    let results = NamedItem::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn uuid_index_filter_new() {
    let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let filter = UuidRecordFilter::new(id, (Utc::now() - Duration::hours(1))..=Utc::now());
    assert_eq!(filter.id, Some(UuidFilter::Equal(id)));
}

#[test]
fn uuid_index_filter_inclusion() {
    let id1 = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let id2 = uuid::Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
    let now = Utc::now();
    let filter = UuidRecordFilter::new([id1, id2], (now - Duration::hours(1))..=now);
    match &filter.id {
        Some(UuidFilter::Inclusion(v)) => assert_eq!(v.len(), 2),
        _ => panic!("Expected Inclusion"),
    }
}

#[test]
fn uuid_index_filter_integration() {
    let id1 = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let id2 = uuid::Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
    let now = Utc::now();

    let data = vec![
        UuidRecord { id: id1, created_at: now, label: "first".into() },
        UuidRecord { id: id2, created_at: now, label: "second".into() },
    ];

    let bytes = UuidRecord::serialize(data).unwrap();

    // Exact match via From<Uuid>
    let filter = UuidRecordFilter::new(id1, (now - Duration::hours(1))..=now + Duration::hours(1));
    let results = UuidRecord::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, id1);
}

#[test]
fn filter_default_and_field_assignment() {
    // Start with default (all None/empty)
    let mut filter = SensorFilter::default();
    assert_eq!(filter.device_id, None);
    assert_eq!(filter.collected_at, None);

    // Set fields individually using From impls
    filter.device_id = Some([1, 2].into()); // Inclusion via From<[i64; N]>
    let now = Utc::now();
    filter.collected_at = Some(((now - Duration::hours(1))..=now).into());

    assert_eq!(filter.device_id, Some(I64Filter::Inclusion(vec![1, 2])));
}

#[test]
fn string_filter_default_and_assignment() {
    let mut filter = NamedItemFilter::default();
    filter.name = Some(("alice").into()); // Equal via From<&str>
    assert_eq!(filter.name, Some(StringFilter::Equal("alice".into())));

    filter.name = Some((["alice", "bob"]).into()); // Inclusion via From<[&str; N]>
    match &filter.name {
        Some(StringFilter::Inclusion(v)) => assert_eq!(v.len(), 2),
        _ => panic!("Expected Inclusion"),
    }
}

#[test]
fn timestamp_filter_equal() {
    let now = Utc::now();
    let mut filter = SensorFilter::default();
    filter.collected_at = Some((now).into());
    assert_eq!(filter.collected_at, Some(DateTimeFilter::Equal(now)));
}

#[test]
fn timestamp_filter_range() {
    let now = Utc::now();
    let mut filter = SensorFilter::default();
    filter.collected_at = Some(((now - Duration::hours(1))..=now).into());
    match &filter.collected_at {
        Some(DateTimeFilter::Range { start, end }) => {
            assert_eq!(start.to_rfc3339(), (now - Duration::hours(1)).to_rfc3339());
            assert_eq!(end.to_rfc3339(), now.to_rfc3339());
        }
        _ => panic!("Expected Range"),
    }
}

#[test]
fn timestamp_filter_inclusion() {
    let t1 = Utc::now();
    let t2 = t1 + Duration::hours(1);
    let mut filter = SensorFilter::default();
    filter.collected_at = Some(([t1, t2]).into());
    match &filter.collected_at {
        Some(DateTimeFilter::Inclusion(v)) => assert_eq!(v.len(), 2),
        _ => panic!("Expected Inclusion"),
    }
}

#[test]
fn timestamp_filter_integration() {
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 23.0 },
        Sensor { device_id: 1, collected_at: now - Duration::minutes(30), temperature: 25.0 },
        Sensor { device_id: 1, collected_at: now + Duration::minutes(5), temperature: 27.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    // Filter by timestamp range only (no index constraint)
    let mut filter = SensorFilter::default();
    filter.collected_at = Some(((now - Duration::minutes(10))..=now).into());

    let results = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
}

#[derive(PcoPack, Debug, PartialEq)]
#[pco_pack(index = [flagged])]
struct FlaggedItem {
    flagged: bool,
    value: i32,
}

type FlaggedItemFilter = <FlaggedItem as PcoPack>::Filter;

#[test]
fn bool_index_filter_equal() {
    let filter = FlaggedItemFilter::new(true);
    assert_eq!(filter.flagged, Some(BoolFilter::Equal(true)));
}

#[test]
fn bool_index_filter_inclusion() {
    let filter = FlaggedItemFilter::new(&[true, false][..]);
    match &filter.flagged {
        Some(BoolFilter::Inclusion(v)) => assert_eq!(v.len(), 2),
        _ => panic!("Expected Inclusion"),
    }
}

#[test]
fn bool_index_filter_integration() {
    let data = vec![
        FlaggedItem { flagged: true, value: 1 },
        FlaggedItem { flagged: false, value: 2 },
        FlaggedItem { flagged: true, value: 3 },
    ];

    let bytes = FlaggedItem::serialize(data).unwrap();

    // Filter by flagged == true
    let filter = FlaggedItemFilter::new(true);
    let results = FlaggedItem::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].value, 1);
    assert_eq!(results[1].value, 3);

    // Filter by flagged == false
    let filter = FlaggedItemFilter::new(false);
    let results = FlaggedItem::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].value, 2);
}

#[test]
fn range_bounds_from_range_filter() {
    let now = Utc::now();
    let start = now - Duration::minutes(10);
    let filter = SensorFilter::new(1i64, start..=now);
    let (s, e) = filter.range_bounds().unwrap();
    assert_eq!(s.to_rfc3339(), start.to_rfc3339());
    assert_eq!(e.to_rfc3339(), now.to_rfc3339());
}

#[test]
fn range_bounds_missing_timestamp() {
    assert!(SensorFilter::default().range_bounds().is_err());
}

#[test]
fn range_bounds_non_range_filter() {
    let now = Utc::now();
    let mut filter = SensorFilter::new(1i64, (Utc::now() - Duration::minutes(10))..=Utc::now());
    filter.collected_at = Some(now.into());
    assert!(filter.range_bounds().is_err());
}

#[test]
fn range_duration_from_range_filter() {
    let now = Utc::now();
    let start = now - Duration::minutes(15);
    let filter = SensorFilter::new(1i64, start..=now);
    let dur = filter.range_duration().unwrap();
    assert_eq!(dur.num_minutes(), 15);
}

#[test]
fn range_shift() {
    let now = Utc::now();
    let start = now - Duration::minutes(10);
    let mut filter = SensorFilter::new(1i64, start..=now);

    // Shift forward by 5 minutes
    filter.range_shift(Duration::minutes(5)).unwrap();
    let (s, e) = filter.range_bounds().unwrap();
    assert_eq!(s.to_rfc3339(), (start + Duration::minutes(5)).to_rfc3339());
    assert_eq!(e.to_rfc3339(), (now + Duration::minutes(5)).to_rfc3339());

    // Shift backward by 20 minutes
    filter.range_shift(Duration::minutes(-20)).unwrap();
    let (s, e) = filter.range_bounds().unwrap();
    assert_eq!(s.to_rfc3339(), (start - Duration::minutes(15)).to_rfc3339());
    assert_eq!(e.to_rfc3339(), (now - Duration::minutes(15)).to_rfc3339());
}

#[test]
fn range_helpers_no_timestamp_field() {
    // QueryStatFilter has no timestamp field, so it shouldn't have range helpers.
    // This test just verifies the filter compiles and works normally.
    let _filter = QueryStatFilter::new(1i64, 5);
}

#[test]
fn typed_plain_field_assignment() {
    // temperature is a plain f64 field; it now has a typed filter accessor.
    let mut filter = SensorFilter::default();
    filter.temperature = Some((20.0..=30.0).into());

    if let Some(F64Filter::Range { start, end }) = &filter.temperature {
        assert_eq!(*start, 20.0);
        assert_eq!(*end, 30.0);
    } else {
        panic!("Expected F64Filter::Range");
    }
}

#[test]
fn typed_plain_field_equal() {
    let mut filter = SensorFilter::default();
    filter.temperature = Some((25.0).into());
    assert_eq!(filter.temperature, Some(F64Filter::Equal(25.0)));
}

#[test]
fn typed_plain_field_inclusion() {
    let mut filter = SensorFilter::default();
    filter.temperature = Some(([20.0, 25.0, 30.0]).into());
    match &filter.temperature {
        Some(F64Filter::Inclusion(v)) => assert_eq!(v.len(), 3),
        _ => panic!("Expected Inclusion"),
    }
}

#[test]
fn typed_plain_field_serializes_to_json() {
    let mut filter = SensorFilter::new(1i64, (Utc::now() - Duration::minutes(10))..=Utc::now());
    filter.temperature = Some((20.0..=30.0).into());

    let json: serde_json::Value = filter.try_into().unwrap();
    assert_eq!(json["temperature"], serde_json::json!({ "start": 20.0, "end": 30.0 }));
}

#[test]
fn typed_plain_field_deserializes_from_json() {
    let json = serde_json::json!({
        "device_id": 1,
        "collected_at": { "start": "2026-01-01T00:00:00Z", "end": "2026-01-01T01:00:00Z" },
        "temperature": { "start": 20.0, "end": 30.0 }
    });

    let filter: SensorFilter = json.try_into().unwrap();
    assert_eq!(filter.device_id, Some(I64Filter::Equal(1)));
    if let Some(F64Filter::Range { start, end }) = &filter.temperature {
        assert_eq!(*start, 20.0);
        assert_eq!(*end, 30.0);
    } else {
        panic!("Expected F64Filter::Range");
    }
    // Verify typed fields are NOT duplicated in others
    assert!(filter.get("device_id").is_none());
    assert!(filter.get("collected_at").is_none());
    assert!(filter.get("temperature").is_none());
}

#[test]
fn typed_plain_field_integration_with_filter_bytes() {
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 23.0 },
        Sensor { device_id: 1, collected_at: now - Duration::minutes(10), temperature: 35.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    // Use typed filter on a plain field via JSON serialization
    let mut filter = SensorFilter::new(1i64, (now - Duration::minutes(15))..=now);
    filter.temperature = Some((20.0..=30.0).into());

    let results = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].temperature, 23.0);
}

#[test]
fn typed_index_field_assignment_integration() {
    // Test that setting an index field via direct accessor (not new()) works with filter_bytes.
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 23.0 },
        Sensor { device_id: 2, collected_at: now - Duration::minutes(5), temperature: 42.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    // Build filter using Default + field assignment instead of new()
    let mut filter = SensorFilter::default();
    filter.device_id = Some([1, 2].into());
    filter.collected_at = Some(((now - Duration::minutes(10))..=now).into());

    let results = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn typed_index_field_assignment_integration_exact() {
    // Test exact match on index field via direct accessor.
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 23.0 },
        Sensor { device_id: 2, collected_at: now - Duration::minutes(5), temperature: 42.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    let mut filter = SensorFilter::default();
    filter.device_id = Some(1.into());
    filter.collected_at = Some(((now - Duration::minutes(10))..=now).into());

    let results = Sensor::filter_bytes(&bytes, filter, &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].device_id, 1);
}

#[test]
fn index_only_query_returns_row_per_chunk() {
    // QueryStat has index = [db_id, granularity], payload = [calls].
    // When we filter on db_id but only request db_id (no payload), we should
    // get back one row per matching chunk with just db_id populated.
    let data = vec![
        QueryStat { db_id: 1, granularity: 60, calls: 100 },
        QueryStat { db_id: 1, granularity: 60, calls: 200 },
        QueryStat { db_id: 2, granularity: 60, calls: 300 },
    ];

    let bytes = QueryStat::serialize(data).unwrap();

    // Filter on db_id = 1, but only request the index field db_id.
    let results = QueryStat::filter_bytes(&bytes, serde_json::json!({"db_id": 1}), &["db_id"]).unwrap();
    // Should return one row per matching chunk (all rows share db_id=1, so one chunk).
    assert_eq!(results.iter().map(|r| (r.db_id, r.granularity)).collect::<Vec<_>>(), vec![(1, 60)]);
}

#[test]
fn index_only_query_with_range_filter() {
    let data = vec![
        QueryStat { db_id: 1, granularity: 60, calls: 100 },
        QueryStat { db_id: 2, granularity: 60, calls: 200 },
        QueryStat { db_id: 3, granularity: 60, calls: 300 },
    ];

    let bytes = QueryStat::serialize(data).unwrap();

    // Filter on db_id range, only request db_id.
    // Range matches chunks with db_id 1 and 2, so we get one row per matching chunk.
    let results =
        QueryStat::filter_bytes(&bytes, serde_json::json!({"db_id": {"start": 1, "end": 2}}), &["db_id"]).unwrap();
    let mut results = results.iter().map(|r| (r.db_id, r.granularity)).collect::<Vec<_>>();
    results.sort();
    assert_eq!(results, vec![(1, 60), (2, 60)]);
}

#[test]
fn single_index_field_query_only() {
    // Sensor has index = [device_id], timestamp = collected_at, payload = [temperature].
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 42, collected_at: now - Duration::minutes(5), temperature: 23.0 },
        Sensor { device_id: 42, collected_at: now - Duration::minutes(3), temperature: 24.0 },
        Sensor { device_id: 99, collected_at: now - Duration::minutes(5), temperature: 30.0 },
    ];

    let bytes = Sensor::serialize(data).unwrap();

    // Filter on device_id, only request device_id (no timestamp, no temperature).
    let results = Sensor::filter_bytes(&bytes, serde_json::json!({"device_id": 42}), &["device_id"]).unwrap();
    assert_eq!(results.iter().map(|r| r.device_id).collect::<Vec<_>>(), vec![42]);
}

#[test]
fn multi_index_query_only_requests_one_index_field() {
    // QueryStat has index = [db_id, granularity].
    let data = vec![
        QueryStat { db_id: 1, granularity: 60, calls: 100 },
        QueryStat { db_id: 1, granularity: 300, calls: 50 },
        QueryStat { db_id: 2, granularity: 60, calls: 75 },
    ];

    let bytes = QueryStat::serialize(data).unwrap();

    // Filter on db_id=1, only request granularity (another index field, no payload).
    // db_id=1 matches two chunks: (1,60) and (1,300), so we get one row per chunk.
    let results = QueryStat::filter_bytes(&bytes, serde_json::json!({"db_id": 1}), &["granularity"]).unwrap();
    let mut results = results.iter().map(|r| (r.db_id, r.granularity)).collect::<Vec<_>>();
    results.sort();
    assert_eq!(results, vec![(1, 60), (1, 300)]);
}

#[test]
fn multi_index_query_only_requests_both_index_fields() {
    // QueryStat has index = [db_id, granularity].
    let data = vec![
        QueryStat { db_id: 1, granularity: 60, calls: 100 },
        QueryStat { db_id: 1, granularity: 300, calls: 50 },
        QueryStat { db_id: 2, granularity: 60, calls: 75 },
    ];

    let bytes = QueryStat::serialize(data).unwrap();

    // Filter on db_id=1, request both index fields (no payload).
    // db_id=1 matches two chunks: (1,60) and (1,300), so we get one row per chunk.
    let results = QueryStat::filter_bytes(&bytes, serde_json::json!({"db_id": 1}), &["db_id", "granularity"]).unwrap();
    let mut results = results.iter().map(|r| (r.db_id, r.granularity)).collect::<Vec<_>>();
    results.sort();
    assert_eq!(results, vec![(1, 60), (1, 300)]);
}

#[test]
fn index_only_query_with_string_index() {
    // NamedItem has index = [name], payload = [count].
    let data = vec![
        NamedItem { name: "alpha".into(), count: 10 },
        NamedItem { name: "beta".into(), count: 20 },
        NamedItem { name: "alpha".into(), count: 30 },
    ];

    let bytes = NamedItem::serialize(data).unwrap();

    // Filter on name, only request name (no payload).
    let results = NamedItem::filter_bytes(&bytes, serde_json::json!({"name": "alpha"}), &["name"]).unwrap();
    assert_eq!(results.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["alpha"]);
}

#[test]
fn index_only_query_with_uuid_index() {
    // UuidRecord has index = [id], timestamp = created_at, payload = [label].
    let id1 = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let id2 = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440002").unwrap();
    let now = Utc::now();
    let data = vec![
        UuidRecord { id: id1, created_at: now - Duration::minutes(5), label: "first".into() },
        UuidRecord { id: id2, created_at: now - Duration::minutes(3), label: "second".into() },
    ];

    let bytes = UuidRecord::serialize(data).unwrap();

    // Filter on id, only request id (no timestamp, no payload).
    let results =
        UuidRecord::filter_bytes(&bytes, serde_json::json!({"id": "550e8400-e29b-41d4-a716-446655440001"}), &["id"])
            .unwrap();
    assert_eq!(results.iter().map(|r| r.id).collect::<Vec<_>>(), vec![id1]);
}

#[test]
fn resolve_fields_includes_start_at_end_at_when_timestamp_requested() {
    // Sensor has index = [device_id], timestamp = collected_at, payload = [temperature].
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 20.0 },
        Sensor { device_id: 1, collected_at: now - Duration::minutes(3), temperature: 21.0 },
    ];
    let chunks = Sensor::write(data).unwrap();

    // Requesting the timestamp field should auto-include start_at and end_at.
    let fields = Sensor::resolve_fields(&serde_json::json!({"device_id": 1}), &["collected_at"]).unwrap();
    assert!(fields.contains(&"collected_at"));
    assert!(fields.contains(&"start_at"));
    assert!(fields.contains(&"end_at"));

    // filter should still work correctly with auto-included fields.
    let results = Sensor::filter(&chunks, serde_json::json!({"device_id": 1}), &["collected_at"]).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn resolve_fields_includes_start_at_end_at_when_timestamp_in_query() {
    // Even if timestamp is only referenced by the query (not explicitly requested),
    // start_at and end_at should be auto-included.
    let now = Utc::now();
    let data = vec![
        Sensor { device_id: 1, collected_at: now - Duration::minutes(5), temperature: 20.0 },
        Sensor { device_id: 1, collected_at: now - Duration::minutes(3), temperature: 21.0 },
    ];
    let chunks = Sensor::write(data).unwrap();

    // Query uses collected_at range; fields only requests device_id.
    let query = serde_json::json!({"collected_at": {"start": "2020-01-01T00:00:00Z", "end": "2030-01-01T00:00:00Z"}});
    let fields = Sensor::resolve_fields(&query, &["device_id"]).unwrap();
    assert!(fields.contains(&"collected_at"));
    assert!(fields.contains(&"start_at"));
    assert!(fields.contains(&"end_at"));

    let results = Sensor::filter(&chunks, query, &["device_id", "collected_at"]).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn resolve_fields_no_start_at_end_at_when_timestamp_not_requested() {
    // If the timestamp field is neither requested nor in the query,
    // start_at and end_at should NOT be auto-included.
    let fields = Sensor::resolve_fields(&serde_json::json!({"device_id": 1}), &["temperature"]).unwrap();
    assert!(fields.contains(&"temperature"));
    assert!(!fields.contains(&"collected_at"));
    assert!(!fields.contains(&"start_at"));
    assert!(!fields.contains(&"end_at"));
}

#[test]
fn resolve_fields_no_start_at_end_at_for_struct_without_timestamp() {
    // QueryStat has no timestamp field, so start_at/end_at should never appear.
    let fields = QueryStat::resolve_fields(&serde_json::json!({"db_id": 1}), &["calls"]).unwrap();
    assert!(fields.contains(&"calls"));
    assert!(!fields.contains(&"start_at"));
    assert!(!fields.contains(&"end_at"));
}

#[test]
fn resolve_fields_start_at_end_at_with_empty_fields_allows_all() {
    // When fields is empty, all known fields are returned (including start_at/end_at if timestamp exists).
    let fields = Sensor::resolve_fields(&serde_json::json!({}), &[]).unwrap();
    assert!(fields.contains(&"device_id"));
    assert!(fields.contains(&"collected_at"));
    assert!(fields.contains(&"temperature"));
    // With empty fields, all struct fields are returned; timestamp is included, so start_at/end_at too.
    assert!(fields.contains(&"start_at"));
    assert!(fields.contains(&"end_at"));
}

#[test]
fn resolve_fields_can_be_called_multiple_times_with_start_at_end_at() {
    // Calling resolve_fields twice with the same query/fields should not error.
    // The second call's fields may include start_at/end_at from the first call's result,
    // so start_at/end_at must be in the known set.
    let query = serde_json::json!({"device_id": 1});

    // First call: request timestamp field
    let fields1 = Sensor::resolve_fields(&query, &["collected_at"]).unwrap();
    assert!(fields1.contains(&"start_at"));
    assert!(fields1.contains(&"end_at"));

    // Second call: pass back the fields from the first call (which includes start_at/end_at).
    // This should not error with "Unknown field: start_at".
    let fields2 = Sensor::resolve_fields(&query, &fields1.iter().copied().collect::<Vec<_>>()).unwrap();
    assert!(fields2.contains(&"collected_at"));
    assert!(fields2.contains(&"start_at"));
    assert!(fields2.contains(&"end_at"));
}

#[test]
fn query_stat_with_timestamp_filter_from_json() {
    let data = vec![
        QueryStatWithTimestamp {
            database_id: 23,
            granularity: 60,
            collected_at: DateTime::parse_from_rfc3339("2026-07-11T21:00:00Z").unwrap().with_timezone(&Utc),
            fingerprint: 100,
        },
        QueryStatWithTimestamp {
            database_id: 24,
            granularity: 60,
            collected_at: DateTime::parse_from_rfc3339("2026-07-11T22:00:00Z").unwrap().with_timezone(&Utc),
            fingerprint: 200,
        },
    ];

    let bytes = QueryStatWithTimestamp::serialize(data).unwrap();

    // Deserialize JSON into typed Filter, then filter_bytes with it.
    let json = serde_json::json!({
        "database_id": 23,
        "collected_at": {
            "start": "2026-07-11T20:19:47Z",
            "end": "2026-07-12T20:19:47Z"
        }
    });

    // Step 1: deserialize JSON into Filter
    let filter: QueryStatWithTimestampFilter = json.clone().try_into().expect("Failed to deserialize JSON into Filter");

    // Step 2: convert Filter -> serde_json::Value via TryInto (this happens inside filter_bytes automatically)
    let roundtrip_json: serde_json::Value = filter.try_into().unwrap_or_else(|e| panic!("Filter -> JSON failed: {e}"));
    eprintln!("Round-trip filter JSON: {}", serde_json::to_string_pretty(&roundtrip_json).unwrap());

    // Step 3: resolve the query from that JSON (this is where bug manifests)
    let resolved = QueryStatWithTimestamp::resolve_query(&roundtrip_json);
    match &resolved {
        Ok(plan) => eprintln!("Resolved filter plan: {plan:#?}"),
        Err(e) => panic!("resolve_query failed: {e:?}"),
    }

    // Step 4: actually call filter_bytes with JSON (same path as passing Filter directly)
    let results = QueryStatWithTimestamp::filter_bytes(&bytes, roundtrip_json.clone(), &[]);
    match results {
        Ok(rows) => {
            eprintln!("Got {} rows", rows.len());
            assert!(!rows.is_empty(), "Expected at least one matching row");
        }
        Err(e) => panic!("filter_bytes failed: {e:?}"),
    }
}

#[test]
fn query_stat_with_timestamp_filter_from_json_microseconds() {
    // Test what happens if we pass microseconds integers instead of RFC3339 strings.
    // This tests a potential edge case where JSON numbers (f64) might corrupt large timestamps
    // due to float precision loss when round-tripping through typed Filter and back.
    let data = vec![QueryStatWithTimestamp {
        database_id: 23,
        granularity: 60,
        collected_at: DateTime::parse_from_rfc3339("2026-07-11T21:00:00Z").unwrap().with_timezone(&Utc),
        fingerprint: 100,
    }];

    let bytes = QueryStatWithTimestamp::serialize(data).unwrap();

    // Pass microseconds as integers directly instead of RFC3339 strings
    let start_micros = 1783801187000000i64;
    let end_micros = 1783887587000000i64;

    let json = serde_json::json!({
        "database_id": 23,
        "collected_at": {
            "start": start_micros,
            "end": end_micros
        }
    });

    // Try to deserialize JSON with microseconds integers into typed Filter
    let result: Result<QueryStatWithTimestampFilter, _> = json.clone().try_into();
    match result {
        Ok(filter) => {
            // Round-trip back to JSON and see what we get
            let roundtrip_json: serde_json::Value =
                filter.try_into().unwrap_or_else(|e| panic!("Filter -> JSON failed: {e}"));

            // Filter with the result
            let results = QueryStatWithTimestamp::filter_bytes(&bytes, roundtrip_json.clone(), &[]);
            match results {
                Ok(rows) => {
                    eprintln!("Got {} rows filtering with microseconds integers", rows.len());
                }
                Err(e) => panic!("filter_bytes failed: {e:?}"),
            }
        }
        Err(e) => {
            // This is OK - DateTimeFilter might not support integer microseconds via serde
            eprintln!("Expected: Cannot deserialize microseconds integers into Filter directly: {}", e);
        }
    }
}

#[test]
fn query_stat_timestamp_filter_partial_match() {
    // Create data with many rows in the same chunk (same index values) spanning a wide time range.
    // Then filter on a narrower timestamp range so some rows match and some don't.
    // This forces actual row-level filtering against decompressed collected_at values.

    let base = DateTime::parse_from_rfc3339("2026-07-10T00:00:00Z").unwrap().with_timezone(&Utc);
    let mut data = Vec::new();
    // Create 10 rows, each hour apart (all with same index values -> single chunk)
    for i in 0..10 {
        data.push(QueryStatWithTimestamp {
            database_id: 23,
            granularity: 60,
            collected_at: base + chrono::Duration::hours(i as i64),
            fingerprint: (i * 10) as i64,
        });
    }

    let bytes = QueryStatWithTimestamp::serialize(data).unwrap();

    // Filter for rows between hour 2 and hour 6 (should match rows at indices 3-5)
    let filter_start = base + chrono::Duration::hours(3);
    let filter_end = base + chrono::Duration::hours(6);

    let json = serde_json::json!({
        "database_id": 23,
        "collected_at": {
            "start": filter_start.to_rfc3339(),
            "end": filter_end.to_rfc3339()
        }
    });

    // Deserialize JSON into typed Filter, then round-trip to JSON and filter (mimics user flow)
    let filter: QueryStatWithTimestampFilter = json.clone().try_into().expect("Failed to deserialize JSON into Filter");

    // Round-trip back to JSON (this happens inside PcoPack::filter)
    let roundtrip_json: serde_json::Value = filter.try_into().unwrap_or_else(|e| panic!("Filter -> JSON failed: {e}"));

    // Filter with the typed filter (via PcoPack::filter_bytes which converts Filter->JSON internally)
    let results = QueryStatWithTimestamp::filter_bytes(&bytes, roundtrip_json.clone(), &[]);
    match results {
        Ok(rows) => {
            assert_eq!(rows.len(), 4, "Expected exactly 4 matching rows (hours 3,4,5,6) but got {}", rows.len());
        }
        Err(e) => {
            panic!("filter_bytes failed: {e:?}");
        }
    }
}

#[test]
fn query_stat_filter_on_chunks_with_timestamp_range() {
    let data = vec![
        QueryStatWithTimestamp {
            database_id: 23,
            granularity: 60,
            collected_at: DateTime::parse_from_rfc3339("2026-07-11T21:00:00Z").unwrap().with_timezone(&Utc),
            fingerprint: 100,
        },
        QueryStatWithTimestamp {
            database_id: 24,
            granularity: 60,
            collected_at: DateTime::parse_from_rfc3339("2026-07-11T22:00:00Z").unwrap().with_timezone(&Utc),
            fingerprint: 200,
        },
    ];

    // Write to chunks first (like PcoPack does internally)
    let chunks = QueryStatWithTimestamp::write(data).expect("Failed to write chunks");

    let json = serde_json::json!({
        "database_id": 23,
        "collected_at": {
            "start": "2026-07-11T20:19:47Z",
            "end": "2026-07-12T20:19:47Z"
        }
    });

    let filter: QueryStatWithTimestampFilter = json.clone().try_into().expect("Failed to deserialize JSON into Filter");

    // Convert Filter -> JSON (happens internally in PcoPack::filter)
    let query_json: serde_json::Value = filter.try_into().unwrap_or_else(|e| panic!("Filter -> JSON failed: {e}"));

    // Call PcoPack::filter on the chunks with this query
    let results = QueryStatWithTimestamp::filter(&chunks, query_json.clone(), &[]);
    match results {
        Ok(rows) => {
            assert!(!rows.is_empty(), "Expected at least one matching row");
        }
        Err(e) => panic!("filter on chunks failed: {e:?}"),
    }
}
