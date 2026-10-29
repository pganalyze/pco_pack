use crate as pco_pack;
use crate::{PcoPack, Resolution, Timeline};
use chrono::{DateTime, Utc};

fn dt(us: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_micros(us).unwrap()
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct TimelineRow {
    seen_at: Timeline,
    value: i32,
}

#[test]
fn timeline_degenerate_range_roundtrip() {
    let tl = Timeline::from_range(dt(1_000_000), dt(1_000_000)); // start == end

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 42 }];
    let bytes = TimelineRow::serialize(data.clone()).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000_000i64, 1_000_000i64)]);
}

#[test]
fn timeline_degenerate_range_filter_exact() {
    let tl = Timeline::from_range(dt(5_000_000), dt(5_000_000));

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();

    let result = TimelineRow::filter_bytes(&bytes, serde_json::json!({"seen_at": 5_000_000}), &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn timeline_degenerate_range_filter_in_range() {
    let tl = Timeline::from_range(dt(5_000_000), dt(5_000_000));

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();

    let result =
        TimelineRow::filter_bytes(&bytes, serde_json::json!({"seen_at": {"start": 4_000_000, "end": 6_000_000}}), &[])
            .unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn timeline_degenerate_range_filter_outside() {
    let tl = Timeline::from_range(dt(5_000_000), dt(5_000_000));

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();

    let result =
        TimelineRow::filter_bytes(&bytes, serde_json::json!({"seen_at": {"start": 6_000_000, "end": 8_000_000}}), &[])
            .unwrap();
    assert!(result.is_empty());
}

#[test]
fn timeline_multiple_overlapping_ranges_merge() {
    let mut tl = Timeline::from_range(dt(1_000), dt(5_000));
    tl.add_range(Resolution::exact(), dt(3_000), dt(7_000)); // overlaps first -> [1k, 7k]
    tl.add_range(Resolution::exact(), dt(6_000), dt(9_000)); // overlaps merged -> [1k, 9k]

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 9_000i64)]);
}

#[test]
fn timeline_non_overlapping_ranges_stay_separate() {
    let mut tl = Timeline::from_range(dt(1_000), dt(2_000));
    tl.add_range(Resolution::exact(), dt(5_000), dt(6_000));

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 2_000i64), (5_000i64, 6_000i64)]);
}

#[test]
fn timeline_containing_range_merge() {
    let mut tl = Timeline::from_range(dt(1_000), dt(10_000)); // wide range
    tl.add_range(Resolution::exact(), dt(3_000), dt(5_000)); // fully inside first

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 10_000i64)]);
}

#[test]
fn timeline_adjacent_ranges_merge() {
    let mut tl = Timeline::from_range(dt(1_000), dt(5_000));
    tl.add_range(Resolution::exact(), dt(5_000), dt(9_000)); // starts exactly where first ends -> adjacent -> merges

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 9_000i64)]);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 10)]
struct ChunkedTimelineRow {
    seen_at: Timeline,
    value: i32,
}

#[test]
fn timeline_many_rows_chunk_boundary() {
    let n = 50;
    let data: Vec<ChunkedTimelineRow> = (0..n)
        .map(|i| {
            let tl = Timeline::from_range(dt((i * 1_000) as i64), dt(((i + 1) * 1_000 - 1) as i64));
            ChunkedTimelineRow { seen_at: tl, value: i as i32 }
        })
        .collect();

    let bytes = ChunkedTimelineRow::serialize(data.clone()).unwrap();
    let result = ChunkedTimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), n);
    for (i, row) in result.iter().enumerate() {
        assert_eq!(row.value, i as i32);
        assert_eq!(row.seen_at.ranges(), &[((i * 1_000) as i64, ((i + 1) * 1_000 - 1) as i64)]);
    }
}

#[test]
fn timeline_many_ranges_per_row_chunk_boundary() {
    let n_rows = 25;
    let ranges_per_row = 40;

    let data: Vec<ChunkedTimelineRow> = (0..n_rows)
        .map(|i| {
            let mut tl = Timeline::default();
            for j in 0..ranges_per_row {
                let base = ((i * ranges_per_row + j) * 1_000) as i64;
                tl.add_range(Resolution::exact(), dt(base), dt(base + 500));
            }
            ChunkedTimelineRow { seen_at: tl, value: i as i32 }
        })
        .collect();

    let bytes = ChunkedTimelineRow::serialize(data.clone()).unwrap();
    let result = ChunkedTimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), n_rows);
    for (i, row) in result.iter().enumerate() {
        assert_eq!(row.value, i as i32);
        assert_eq!(row.seen_at.ranges().len(), ranges_per_row);
    }
}

#[test]
fn timeline_filter_across_chunks_many_ranges() {
    let n_rows = 30;
    let data: Vec<ChunkedTimelineRow> = (0..n_rows)
        .map(|i| {
            let tl = Timeline::from_range(dt((i * 1_000_000) as i64), dt(((i + 1) * 1_000_000 - 1) as i64));
            ChunkedTimelineRow { seen_at: tl, value: i }
        })
        .collect();

    let bytes = ChunkedTimelineRow::serialize(data).unwrap();

    let result = ChunkedTimelineRow::filter_bytes(
        &bytes,
        serde_json::json!({"seen_at": {"start": 5_000_000, "end": 7_000_000}}),
        &[],
    )
    .unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, 5);
    assert_eq!(result[1].value, 6);
    assert_eq!(result[2].value, 7);
}

#[test]
fn timeline_thousands_of_ranges_single_row() {
    let mut tl = Timeline::default();
    for i in 0..5_000 {
        let base = (i * 1_000) as i64;
        tl.add_range(Resolution::exact(), dt(base), dt(base + 100));
    }

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 42 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges().len(), 5_000);
}

// Bucket-mode edge cases

#[test]
fn timeline_bucket_mode_many_events_same_bucket() {
    let mut tl = Timeline::default();
    for i in 0..1_000 {
        tl.add_range(10, dt(i * 1_000), dt(i * 1_000 + 100)); // all within [0, 10M)
    }

    assert_eq!(tl.ranges(), &[(0i64, 9_999_999)]);
}

#[test]
fn timeline_bucket_mode_many_buckets() {
    let mut tl = Timeline::default(); // 1-second buckets
    for i in 0..1_000 {
        tl.add_range(1, dt((i * 1_000_000) as i64 + 500_000), dt((i * 1_000_000) as i64 + 600_000));
    }

    // Adjacent buckets merge into one continuous range
    assert_eq!(tl.ranges(), &[(0i64, 999_999_999)]);
}

#[test]
fn timeline_bucket_mode_range_fills_start_bucket() {
    let mut tl = Timeline::default();
    tl.add_range(10, dt(3_000_000), dt(5_000_000));
    assert_eq!(tl.ranges(), &[(0i64, 9_999_999)]);
}

#[test]
fn timeline_bucket_mode_inverted_range_is_noop() {
    let mut tl = Timeline::default();
    tl.add_range(10, dt(5_000_000), dt(3_000_000));
    assert!(tl.is_empty());
}

#[test]
fn timeline_from_range_is_exact() {
    let tl = Timeline::from_range(dt(3_000_000), dt(5_000_000));
    assert_eq!(tl.ranges(), &[(3_000_000i64, 5_000_000)]);
    let inverted = Timeline::from_range(dt(5_000_000), dt(3_000_000));
    assert!(inverted.is_empty());
}

// Untrusted-input hardening: the stored invariant survives Deserialize, the
// column reader, and inverted query windows.

#[test]
fn timeline_overlaps_inverted_window_is_false() {
    let tl = Timeline::from_range(dt(0), dt(300));
    assert!(tl.overlaps(dt(100), dt(200)));
    assert!(!tl.overlaps(dt(200), dt(50)));
}

#[test]
fn timeline_inverted_json_range_filter_matches_nothing() {
    let data = vec![TimelineRow { seen_at: Timeline::from_range(dt(0), dt(300)), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result =
        TimelineRow::filter_bytes(&bytes, serde_json::json!({"seen_at": {"start": 200, "end": 50}}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn deserialize_drops_inverted_and_coalesces_garbage() {
    let tl: Timeline = serde_json::from_str(r#"{"ranges":[[9,7],[5,6],[0,1],[4,8]]}"#).unwrap();
    assert_eq!(tl.ranges(), &[(0i64, 1), (4, 8)]);
    assert!(!tl.contains(dt(9)));
}

#[test]
fn deserialize_roundtrip_preserves_clean_timeline() {
    let mut tl = Timeline::default();
    tl.add_range(Resolution::exact(), dt(1_000), dt(2_000));
    tl.add_range(Resolution::exact(), dt(5_000), dt(6_000));
    let json = serde_json::to_string(&tl).unwrap();
    let back: Timeline = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tl);
}

#[test]
fn merge_survives_extreme_i64_bounds() {
    let mut dst = Timeline::from_range(dt(0), dt(100));
    dst.add_range(Resolution::exact(), dt(500), dt(600));
    let span_all: Timeline = serde_json::from_str(&format!(r#"{{"ranges":[[{},{}]]}}"#, i64::MIN, i64::MAX)).unwrap();
    dst.merge(&span_all);
    assert_eq!(dst.ranges(), &[(i64::MIN, i64::MAX)]);

    let mut dst = Timeline::from_range(dt(0), dt(100));
    let tail: Timeline = serde_json::from_str(&format!(r#"{{"ranges":[[200,{}]]}}"#, i64::MAX)).unwrap();
    dst.merge(&tail);
    assert_eq!(dst.ranges(), &[(0, 100), (200, i64::MAX)]);

    let mut dst = Timeline::from_range(dt(0), dt(100));
    let head: Timeline = serde_json::from_str(&format!(r#"{{"ranges":[[{},-500]]}}"#, i64::MIN)).unwrap();
    dst.merge(&head);
    assert_eq!(dst.ranges(), &[(i64::MIN, -500), (0, 100)]);
}
