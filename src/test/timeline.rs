use crate as pco_pack;
use crate::{PcoPack, Timeline};

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct TimelineTimestamp {
    seen_at: Timeline<0>,
    value: i64,
}

#[test]
fn timeline_as_timestamp_roundtrip() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(3_000_000, 4_000_000);
    let mut tl3 = Timeline::<0>::new();
    tl3.add(5_000_000, 6_000_000);

    let data = vec![
        TimelineTimestamp { seen_at: tl1, value: 10 },
        TimelineTimestamp { seen_at: tl2, value: 20 },
        TimelineTimestamp { seen_at: tl3, value: 30 },
    ];

    let bytes = TimelineTimestamp::serialize(data.clone()).unwrap();
    let result = TimelineTimestamp::deserialize(&bytes).unwrap();
    assert_eq!(result, data);
}

#[test]
fn timeline_as_timestamp_sorts_by_start() {
    let mut tl_a = Timeline::<0>::new();
    tl_a.add(5_000_000, 6_000_000);
    let mut tl_b = Timeline::<0>::new();
    tl_b.add(1_000_000, 2_000_000);
    let mut tl_c = Timeline::<0>::new();
    tl_c.add(3_000_000, 4_000_000);

    let data = vec![
        TimelineTimestamp { seen_at: tl_a, value: 1 },
        TimelineTimestamp { seen_at: tl_b, value: 2 },
        TimelineTimestamp { seen_at: tl_c, value: 3 },
    ];

    let bytes = TimelineTimestamp::serialize(data).unwrap();
    let result = TimelineTimestamp::deserialize(&bytes).unwrap();
    assert_eq!(result[0].value, 2); // start 1M
    assert_eq!(result[1].value, 3); // start 3M
    assert_eq!(result[2].value, 1); // start 5M
}

#[test]
fn timeline_as_timestamp_empty_timeline_rejected() {
    let data = vec![TimelineTimestamp { seen_at: Timeline::<0>::new(), value: 1 }];

    let err = TimelineTimestamp::serialize(data).unwrap_err();
    assert!(err.to_string().contains("empty Timeline"));
}

#[test]
fn timeline_as_timestamp_multi_range() {
    let mut tl = Timeline::<0>::new();
    tl.add(100, 200);
    tl.add(300, 400);
    tl.add(500, 600);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 42 }];
    let bytes = TimelineTimestamp::serialize(data.clone()).unwrap();
    let result = TimelineTimestamp::deserialize(&bytes).unwrap();
    assert_eq!(result, data);
    assert_eq!(result[0].seen_at.ranges(), &[(100i64, 200), (300, 400), (500, 600)]);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at, index = [device_id])]
struct TimelineWithIndex {
    device_id: i64,
    seen_at: Timeline<0>,
    temperature: f64,
}

#[test]
fn timeline_with_index_merges_identical_rows() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(3_000_000, 4_000_000);

    let data = vec![
        TimelineWithIndex { device_id: 1, seen_at: tl1, temperature: 23.5 },
        TimelineWithIndex { device_id: 1, seen_at: tl2, temperature: 23.5 },
    ];

    let bytes = TimelineWithIndex::serialize(data).unwrap();
    let result = TimelineWithIndex::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].device_id, 1);
    assert_eq!(result[0].temperature, 23.5);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000_000i64, 2_000_000), (3_000_000, 4_000_000)]);
}

#[test]
fn timeline_with_index_no_merge_different_payload() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(3_000_000, 4_000_000);

    let data = vec![
        TimelineWithIndex { device_id: 1, seen_at: tl1, temperature: 23.5 },
        TimelineWithIndex { device_id: 1, seen_at: tl2, temperature: 25.0 },
    ];

    let bytes = TimelineWithIndex::serialize(data.clone()).unwrap();
    let result = TimelineWithIndex::deserialize(&bytes).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn timeline_with_index_different_devices() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(3_000_000, 4_000_000);

    let data = vec![
        TimelineWithIndex { device_id: 1, seen_at: tl1, temperature: 23.5 },
        TimelineWithIndex { device_id: 2, seen_at: tl2, temperature: 23.5 },
    ];

    let bytes = TimelineWithIndex::serialize(data.clone()).unwrap();
    let result = TimelineWithIndex::deserialize(&bytes).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn timeline_filter_range() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(5_000_000, 6_000_000);
    let mut tl3 = Timeline::<0>::new();
    tl3.add(9_000_000, 10_000_000);

    let data = vec![
        TimelineTimestamp { seen_at: tl1, value: 1 },
        TimelineTimestamp { seen_at: tl2, value: 2 },
        TimelineTimestamp { seen_at: tl3, value: 3 },
    ];

    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 4_000_000, "end": 7_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 2);
}

#[test]
fn timeline_filter_exact_point() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(5_000_000, 6_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl1, value: 1 }, TimelineTimestamp { seen_at: tl2, value: 2 }];

    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": 1_500_000});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 1);
}

#[test]
fn timeline_filter_inclusion() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(5_000_000, 6_000_000);
    let mut tl3 = Timeline::<0>::new();
    tl3.add(9_000_000, 10_000_000);

    let data = vec![
        TimelineTimestamp { seen_at: tl1, value: 1 },
        TimelineTimestamp { seen_at: tl2, value: 2 },
        TimelineTimestamp { seen_at: tl3, value: 3 },
    ];

    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": [1_500_000, 9_500_000]});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value, 1);
    assert_eq!(result[1].value, 3);
}

#[test]
fn timeline_filter_no_match() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000_000, 2_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 1 }];
    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 5_000_000, "end": 6_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn timeline_range_filter_skips_chunks() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(100_000_000, 101_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl1, value: 1 }, TimelineTimestamp { seen_at: tl2, value: 2 }];

    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 99_000_000, "end": 102_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 2);
}

// Bucket-mode tests (RESOLUTION > 0)

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct TimelineBucketed {
    seen_at: Timeline<10_000_000>, // 10-second buckets
    value: i64,
}

#[test]
fn timeline_bucket_mode_floors_to_bucket() {
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(3_000_000, 5_000_000); // floors to [0, 10M)

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 10_000_000)]);
}

#[test]
fn timeline_bucket_mode_same_bucket_no_duplicate() {
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(3_000_000, 5_000_000); // bucket [0, 10M)
    tl.add(7_000_000, 9_000_000); // same bucket -> no new range

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 10_000_000)]);
}

#[test]
fn timeline_bucket_mode_adjacent_buckets_merge() {
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(3_000_000, 5_000_000); // bucket [0, 10M)
    tl.add(15_000_000, 18_000_000); // bucket [10M, 20M), adjacent -> merges

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 20_000_000)]);
}

#[test]
fn timeline_bucket_mode_gap_buckets_stay_separate() {
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(3_000_000, 5_000_000); // bucket [0, 10M)
    tl.add(55_000_000, 58_000_000); // bucket [50M, 60M), gap -> separate

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 10_000_000), (50_000_000, 60_000_000)]);
}

#[test]
fn timeline_bucket_mode_negative_timestamps() {
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(-15_000_000, -5_000_000); // floors to [-20M, -10M)

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(-20_000_000i64, -10_000_000)]);
}

#[test]
fn timeline_bucket_mode_negative_crosses_zero() {
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(-5_000_000, -1_000_000); // bucket [-10M, 0)
    tl.add(3_000_000, 5_000_000); // bucket [0, 10M), adjacent -> merges

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(-10_000_000i64, 10_000_000)]);
}

#[test]
fn timeline_partial_read_exclude_value() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000_000, 2_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 42 }];
    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &["seen_at"]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000_000i64, 2_000_000)]);
    assert_eq!(result[0].value, 0); // default
}

#[test]
fn timeline_index_range_filter() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(5_000_000, 6_000_000);
    let mut tl3 = Timeline::<0>::new();
    tl3.add(9_000_000, 10_000_000);

    let data = vec![
        TimelineWithIndex { device_id: 1, seen_at: tl1, temperature: 23.5 },
        TimelineWithIndex { device_id: 1, seen_at: tl2, temperature: 25.0 },
        TimelineWithIndex { device_id: 2, seen_at: tl3, temperature: 20.0 },
    ];

    let bytes = TimelineWithIndex::serialize(data).unwrap();

    let query = serde_json::json!({
        "device_id": 1,
        "seen_at": {"start": 4_000_000, "end": 7_000_000}
    });
    let result = TimelineWithIndex::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].device_id, 1);
    assert_eq!(result[0].temperature, 25.0);
}

#[test]
fn timeline_filter_hits_one_sub_range() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000_000, 2_000_000);
    tl.add(5_000_000, 6_000_000);
    tl.add(9_000_000, 10_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 42 }];
    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 4_000_000, "end": 7_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 42);
}

#[test]
fn timeline_filter_hits_multiple_sub_ranges() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000_000, 2_000_000);
    tl.add(5_000_000, 6_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 42 }];
    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 0, "end": 10_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn timeline_overlapping_ranges_merge_on_add() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000_000, 3_000_000);
    tl.add(2_000_000, 4_000_000); // overlaps with first

    let data = vec![TimelineTimestamp { seen_at: tl, value: 1 }];
    let bytes = TimelineTimestamp::serialize(data.clone()).unwrap();
    let result = TimelineTimestamp::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000_000i64, 4_000_000)]);
}

#[test]
fn timeline_negative_timestamps() {
    let mut tl = Timeline::<0>::new();
    tl.add(-5_000_000, -1_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 1 }];
    let bytes = TimelineTimestamp::serialize(data.clone()).unwrap();
    let result = TimelineTimestamp::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(-5_000_000i64, -1_000_000)]);
}

#[test]
fn timeline_negative_timestamp_filter() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(-5_000_000, -1_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(1_000_000, 2_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl1, value: 1 }, TimelineTimestamp { seen_at: tl2, value: 2 }];

    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": -10_000_000, "end": 0}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 1);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct TimelineAsPayload {
    id: i64,
    seen_at: Timeline<0>,
    label: String,
}

#[test]
fn timeline_as_plain_field_roundtrip() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000_000, 2_000_000);
    tl.add(5_000_000, 6_000_000);

    let data = vec![TimelineAsPayload { id: 1, seen_at: tl, label: "test".into() }];
    let bytes = TimelineAsPayload::serialize(data.clone()).unwrap();
    let result = TimelineAsPayload::deserialize(&bytes).unwrap();
    assert_eq!(result, data);
}

#[test]
fn timeline_as_plain_field_filter_range() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(5_000_000, 6_000_000);

    let data = vec![
        TimelineAsPayload { id: 1, seen_at: tl1, label: "a".into() },
        TimelineAsPayload { id: 2, seen_at: tl2, label: "b".into() },
    ];

    let bytes = TimelineAsPayload::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 4_000_000, "end": 7_000_000}});
    let result = TimelineAsPayload::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
}

#[test]
fn timeline_index_range_filter_skips_other_groups() {
    let mut tl1 = Timeline::<0>::new();
    tl1.add(1_000_000, 2_000_000);
    let mut tl2 = Timeline::<0>::new();
    tl2.add(100_000_000, 101_000_000);

    let data = vec![
        TimelineWithIndex { device_id: 1, seen_at: tl1, temperature: 23.5 },
        TimelineWithIndex { device_id: 2, seen_at: tl2, temperature: 25.0 },
    ];

    let bytes = TimelineWithIndex::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 99_000_000, "end": 102_000_000}});
    let result = TimelineWithIndex::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].device_id, 2);
}

// Edge cases for bucket mode

#[test]
fn timeline_bucket_mode_degenerate_range_becomes_full_bucket() {
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(6_000_000, 6_000_000); // point -> full bucket [0, 10M)

    assert_eq!(tl.ranges(), &[(0i64, 10_000_000)]);
}

#[test]
fn timeline_bucket_mode_range_spanning_buckets() {
    // A range spanning multiple buckets only records the bucket of its start.
    // The semantics are: "something happened at start", so we record that bucket.
    let mut tl = Timeline::<10_000_000>::new();
    tl.add(8_000_000, 25_000_000); // spans [0,10M), [10M,20M), [20M,30M) -> only bucket of start: [0,10M)

    assert_eq!(tl.ranges(), &[(0i64, 10_000_000)]);
}

#[test]
fn timeline_bucket_mode_filter_on_bucketed_ranges() {
    let mut tl1 = Timeline::<10_000_000>::new();
    tl1.add(6_000_000, 9_000_000); // bucket [0, 10M)

    let mut tl2 = Timeline::<10_000_000>::new();
    tl2.add(55_000_000, 60_000_000); // bucket [50M, 60M)

    let data = vec![TimelineBucketed { seen_at: tl1, value: 1 }, TimelineBucketed { seen_at: tl2, value: 2 }];
    let bytes = TimelineBucketed::serialize(data).unwrap();

    // Query that overlaps bucket [0, 10M)
    let result = TimelineBucketed::filter_bytes(&bytes, serde_json::json!({"seen_at": 5_000_000}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 1);

    // Query that overlaps bucket [50M, 60M)
    let result = TimelineBucketed::filter_bytes(
        &bytes,
        serde_json::json!({"seen_at": {"start": 45_000_000, "end": 65_000_000}}),
        &[],
    )
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 2);
}
