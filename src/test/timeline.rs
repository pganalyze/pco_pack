use crate as pco_pack;
use crate::{PcoPack, Resolution, Timeline};

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct TimelineTimestamp {
    seen_at: Timeline,
    value: i64,
}

#[test]
fn timeline_as_timestamp_roundtrip() {
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(3_000_000, 4_000_000);
    let tl3 = Timeline::from_range(5_000_000, 6_000_000);

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
    let tl_a = Timeline::from_range(5_000_000, 6_000_000);
    let tl_b = Timeline::from_range(1_000_000, 2_000_000);
    let tl_c = Timeline::from_range(3_000_000, 4_000_000);

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
    let data = vec![TimelineTimestamp { seen_at: Timeline::default(), value: 1 }];

    let err = TimelineTimestamp::serialize(data).unwrap_err();
    assert!(err.to_string().contains("empty Timeline"));
}

#[test]
fn timeline_as_timestamp_multi_range() {
    let mut tl = Timeline::from_range(100, 200);
    tl.add_range(Resolution::exact(), 300, 400);
    tl.add_range(Resolution::exact(), 500, 600);

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
    seen_at: Timeline,
    temperature: f64,
}

#[test]
fn timeline_with_index_merges_identical_rows() {
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(3_000_000, 4_000_000);

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
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(3_000_000, 4_000_000);

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
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(3_000_000, 4_000_000);

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
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(5_000_000, 6_000_000);
    let tl3 = Timeline::from_range(9_000_000, 10_000_000);

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
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(5_000_000, 6_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl1, value: 1 }, TimelineTimestamp { seen_at: tl2, value: 2 }];

    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": 1_500_000});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 1);
}

#[test]
fn timeline_filter_inclusion() {
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(5_000_000, 6_000_000);
    let tl3 = Timeline::from_range(9_000_000, 10_000_000);

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
    let tl = Timeline::from_range(1_000_000, 2_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 1 }];
    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 5_000_000, "end": 6_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn timeline_range_filter_skips_chunks() {
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(100_000_000, 101_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl1, value: 1 }, TimelineTimestamp { seen_at: tl2, value: 2 }];

    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 99_000_000, "end": 102_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 2);
}

// Bucket-mode tests (resolution > 0)

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct TimelineBucketed {
    seen_at: Timeline, // 10-second buckets
    value: i64,
}

#[test]
fn timeline_bucket_mode_floors_to_bucket() {
    let mut tl = Timeline::default();
    tl.add_range(10, 3_000_000, 5_000_000); // floors to [0, 10M)

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 9_999_999)]);
}

#[test]
fn timeline_bucket_mode_same_bucket_no_duplicate() {
    let mut tl = Timeline::default();
    tl.add_range(10, 3_000_000, 5_000_000); // bucket [0, 10M)
    tl.add_range(10, 7_000_000, 9_000_000); // same bucket -> no new range

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 9_999_999)]);
}

#[test]
fn timeline_bucket_mode_adjacent_buckets_merge() {
    let mut tl = Timeline::default();
    tl.add_range(10, 3_000_000, 5_000_000); // bucket [0, 10M)
    tl.add_range(10, 15_000_000, 18_000_000); // bucket [10M, 20M), adjacent -> merges

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 19_999_999)]);
}

#[test]
fn timeline_bucket_mode_gap_buckets_stay_separate() {
    let mut tl = Timeline::default();
    tl.add_range(10, 3_000_000, 5_000_000); // bucket [0, 10M)
    tl.add_range(10, 55_000_000, 58_000_000); // bucket [50M, 60M), gap -> separate

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(0i64, 9_999_999), (50_000_000, 59_999_999)]);
}

#[test]
fn timeline_bucket_mode_negative_timestamps() {
    let mut tl = Timeline::default();
    tl.add_range(10, -15_000_000, -5_000_000); // spans buckets [-20M, -10M) and [-10M, 0)

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(-20_000_000i64, -1)]);
}

#[test]
fn timeline_bucket_mode_negative_crosses_zero() {
    let mut tl = Timeline::default();
    tl.add_range(10, -5_000_000, -1_000_000); // bucket [-10M, 0)
    tl.add_range(10, 3_000_000, 5_000_000); // bucket [0, 10M), adjacent -> merges

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data.clone()).unwrap();
    let result = TimelineBucketed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(-10_000_000i64, 9_999_999)]);
}

#[test]
fn timeline_partial_read_exclude_value() {
    let tl = Timeline::from_range(1_000_000, 2_000_000);

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
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(5_000_000, 6_000_000);
    let tl3 = Timeline::from_range(9_000_000, 10_000_000);

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
    let mut tl = Timeline::from_range(1_000_000, 2_000_000);
    tl.add_range(Resolution::exact(), 5_000_000, 6_000_000);
    tl.add_range(Resolution::exact(), 9_000_000, 10_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 42 }];
    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 4_000_000, "end": 7_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 42);
}

#[test]
fn timeline_filter_hits_multiple_sub_ranges() {
    let mut tl = Timeline::from_range(1_000_000, 2_000_000);
    tl.add_range(Resolution::exact(), 5_000_000, 6_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 42 }];
    let bytes = TimelineTimestamp::serialize(data).unwrap();

    let query = serde_json::json!({"seen_at": {"start": 0, "end": 10_000_000}});
    let result = TimelineTimestamp::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn timeline_overlapping_ranges_merge_on_add() {
    let mut tl = Timeline::from_range(1_000_000, 3_000_000);
    tl.add_range(Resolution::exact(), 2_000_000, 4_000_000); // overlaps with first

    let data = vec![TimelineTimestamp { seen_at: tl, value: 1 }];
    let bytes = TimelineTimestamp::serialize(data.clone()).unwrap();
    let result = TimelineTimestamp::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000_000i64, 4_000_000)]);
}

#[test]
fn timeline_negative_timestamps() {
    let tl = Timeline::from_range(-5_000_000, -1_000_000);

    let data = vec![TimelineTimestamp { seen_at: tl, value: 1 }];
    let bytes = TimelineTimestamp::serialize(data.clone()).unwrap();
    let result = TimelineTimestamp::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(-5_000_000i64, -1_000_000)]);
}

#[test]
fn timeline_negative_timestamp_filter() {
    let tl1 = Timeline::from_range(-5_000_000, -1_000_000);
    let tl2 = Timeline::from_range(1_000_000, 2_000_000);

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
    seen_at: Timeline,
    label: String,
}

#[test]
fn timeline_filter_type_error_includes_field_and_type() {
    let tl = Timeline::from_range(1_000, 2_000);
    let data = vec![TimelineAsPayload { id: 1, seen_at: tl, label: "a".into() }];
    let bytes = TimelineAsPayload::serialize(data).unwrap();

    let result = TimelineAsPayload::filter_bytes(&bytes, serde_json::json!({"seen_at": "nope"}), &[]);
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to resolve filter for field 'seen_at'"), "error message: {err}");
    assert!(err.contains("Expected integer microseconds, got string"), "error message: {err}");

    let result =
        TimelineAsPayload::filter_bytes(&bytes, serde_json::json!({"seen_at": {"start": true, "end": 2}}), &[]);
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Range start must be an integer, got boolean"), "error message: {err}");
}

#[test]
fn timeline_as_plain_field_roundtrip() {
    let mut tl = Timeline::from_range(1_000_000, 2_000_000);
    tl.add_range(Resolution::exact(), 5_000_000, 6_000_000);

    let data = vec![TimelineAsPayload { id: 1, seen_at: tl, label: "test".into() }];
    let bytes = TimelineAsPayload::serialize(data.clone()).unwrap();
    let result = TimelineAsPayload::deserialize(&bytes).unwrap();
    assert_eq!(result, data);
}

#[test]
fn timeline_as_plain_field_filter_range() {
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(5_000_000, 6_000_000);

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
    let tl1 = Timeline::from_range(1_000_000, 2_000_000);
    let tl2 = Timeline::from_range(100_000_000, 101_000_000);

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
    let mut tl = Timeline::default();
    tl.add(10, 6_000_000); // point -> full bucket [0, 10M)

    assert_eq!(tl.ranges(), &[(0i64, 9_999_999)]);
}

#[test]
fn timeline_bucket_mode_range_spanning_buckets() {
    // A range spanning multiple buckets fills every bucket it overlaps.
    let mut tl = Timeline::default();
    tl.add_range(10, 8_000_000, 25_000_000); // spans [0,10M), [10M,20M), [20M,30M) -> merges to one

    assert_eq!(tl.ranges(), &[(0i64, 29_999_999)]);
}

#[test]
fn timeline_bucket_boundary_belongs_to_next_bucket() {
    let mut tl = Timeline::default();
    tl.add(10, 5_000_000); // bucket [0, 10M)
    assert_eq!(tl.ranges(), &[(0i64, 9_999_999)]);
    assert!(tl.contains(9_999_999));
    assert!(!tl.contains(10_000_000));

    let data = vec![TimelineBucketed { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineBucketed::serialize(data).unwrap();

    // Queries starting at the next bucket's boundary must not match a timeline
    // that only covers [0, 10M).
    let result = TimelineBucketed::filter_bytes(&bytes, serde_json::json!({"seen_at": 10_000_000}), &[]).unwrap();
    assert!(result.is_empty());
    let result = TimelineBucketed::filter_bytes(
        &bytes,
        serde_json::json!({"seen_at": {"start": 10_000_000, "end": 15_000_000}}),
        &[],
    )
    .unwrap();
    assert!(result.is_empty());
    let result = TimelineBucketed::filter_bytes(&bytes, serde_json::json!({"seen_at": 9_999_999}), &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn timeline_bucket_mode_filter_on_bucketed_ranges() {
    let mut tl1 = Timeline::default();
    tl1.add_range(10, 6_000_000, 9_000_000); // bucket [0, 10M)

    let mut tl2 = Timeline::default();
    tl2.add_range(10, 55_000_000, 60_000_000); // ends on a boundary -> fills [50M, 70M)

    let data = vec![TimelineBucketed { seen_at: tl1, value: 1 }, TimelineBucketed { seen_at: tl2, value: 2 }];
    let bytes = TimelineBucketed::serialize(data).unwrap();

    // Query that overlaps bucket [0, 10M)
    let result = TimelineBucketed::filter_bytes(&bytes, serde_json::json!({"seen_at": 5_000_000}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 1);

    // Query that overlaps the 50M-side ranges
    let result = TimelineBucketed::filter_bytes(
        &bytes,
        serde_json::json!({"seen_at": {"start": 45_000_000, "end": 65_000_000}}),
        &[],
    )
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 2);
}

#[test]
fn timeline_resolution_units_agree() {
    // A bare integer, Resolution::seconds, and a chrono Duration of equal width
    // produce identical bucketing.
    let mut a = Timeline::default();
    let mut b = Timeline::default();
    let mut c = Timeline::default();
    for ts in [3_000_000i64, 55_000_000, 95_000_000] {
        a.add(10, ts);
        b.add(Resolution::seconds(10), ts);
        c.add(chrono::Duration::seconds(10), ts);
    }
    assert_eq!(a.ranges(), b.ranges());
    assert_eq!(b.ranges(), c.ranges());
    assert_eq!(a.ranges(), &[(0i64, 9_999_999), (50_000_000, 59_999_999), (90_000_000, 99_999_999)]);
}

#[test]
fn timeline_sub_second_resolution_via_duration() {
    let mut tl = Timeline::default();
    tl.add(chrono::Duration::milliseconds(500), 1_200_000); // bucket [1.0M, 1.5M)
    tl.add(chrono::Duration::milliseconds(500), 1_700_000); // bucket [1.5M, 2.0M), adjacent -> merges
    assert_eq!(tl.ranges(), &[(1_000_000i64, 1_999_999)]);
}

#[test]
fn timeline_mixed_resolutions_never_rebucket() {
    // Recording an observation observed at a coarse cadence then finer ones on the
    // same timeline keeps each observation's own bucket width.
    let mut tl = Timeline::default();
    tl.add(10, 3_000_000); // [0, 10M)
    tl.add(1, 5_000_000); // [5M, 6M) contained -> no change
    tl.add(1, 10_000_000); // [10M, 11M) adjacent -> merges
    assert_eq!(tl.ranges(), &[(0i64, 10_999_999)]);
}

#[test]
#[should_panic(expected = "non-negative")]
fn timeline_negative_resolution_panics() {
    let mut tl = Timeline::default();
    tl.add(-1i32, 5_000_000);
}
