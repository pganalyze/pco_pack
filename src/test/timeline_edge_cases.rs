use crate as pco_pack;
use crate::{PcoPack, Timeline};

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct TimelineRow {
    seen_at: Timeline<0>,
    value: i32,
}

#[test]
fn timeline_degenerate_range_roundtrip() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000_000, 1_000_000); // start == end

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 42 }];
    let bytes = TimelineRow::serialize(data.clone()).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000_000i64, 1_000_000i64)]);
}

#[test]
fn timeline_degenerate_range_filter_exact() {
    let mut tl = Timeline::<0>::new();
    tl.add(5_000_000, 5_000_000);

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();

    let result = TimelineRow::filter_bytes(&bytes, serde_json::json!({"seen_at": 5_000_000}), &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn timeline_degenerate_range_filter_in_range() {
    let mut tl = Timeline::<0>::new();
    tl.add(5_000_000, 5_000_000);

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();

    let result =
        TimelineRow::filter_bytes(&bytes, serde_json::json!({"seen_at": {"start": 4_000_000, "end": 6_000_000}}), &[])
            .unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn timeline_degenerate_range_filter_outside() {
    let mut tl = Timeline::<0>::new();
    tl.add(5_000_000, 5_000_000);

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();

    let result =
        TimelineRow::filter_bytes(&bytes, serde_json::json!({"seen_at": {"start": 6_000_000, "end": 8_000_000}}), &[])
            .unwrap();
    assert!(result.is_empty());
}

#[test]
fn timeline_multiple_overlapping_ranges_merge() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000, 5_000);
    tl.add(3_000, 7_000); // overlaps first -> [1k, 7k]
    tl.add(6_000, 9_000); // overlaps merged -> [1k, 9k]

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 9_000i64)]);
}

#[test]
fn timeline_non_overlapping_ranges_stay_separate() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000, 2_000);
    tl.add(5_000, 6_000);

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 2_000i64), (5_000i64, 6_000i64)]);
}

#[test]
fn timeline_containing_range_merge() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000, 10_000); // wide range
    tl.add(3_000, 5_000); // fully inside first

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 10_000i64)]);
}

#[test]
fn timeline_adjacent_ranges_merge() {
    let mut tl = Timeline::<0>::new();
    tl.add(1_000, 5_000);
    tl.add(5_000, 9_000); // starts exactly where first ends -> adjacent -> merges

    let data = vec![TimelineRow { seen_at: tl.clone(), value: 1 }];
    let bytes = TimelineRow::serialize(data).unwrap();
    let result = TimelineRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].seen_at.ranges(), &[(1_000i64, 9_000i64)]);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(chunk_size = 10)]
struct ChunkedTimelineRow {
    seen_at: Timeline<0>,
    value: i32,
}

#[test]
fn timeline_many_rows_chunk_boundary() {
    let n = 50;
    let data: Vec<ChunkedTimelineRow> = (0..n)
        .map(|i| {
            let mut tl = Timeline::<0>::new();
            tl.add((i * 1_000) as i64, ((i + 1) * 1_000 - 1) as i64);
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
            let mut tl = Timeline::<0>::new();
            for j in 0..ranges_per_row {
                let base = ((i * ranges_per_row + j) * 1_000) as i64;
                tl.add(base, base + 500);
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
            let mut tl = Timeline::<0>::new();
            tl.add((i * 1_000_000) as i64, ((i + 1) * 1_000_000 - 1) as i64);
            ChunkedTimelineRow { seen_at: tl, value: i as i32 }
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
    let mut tl = Timeline::<0>::new();
    for i in 0..5_000 {
        let base = (i * 1_000) as i64;
        tl.add(base, base + 100);
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
    let mut tl = Timeline::<10_000_000>::new();
    for i in 0..1_000 {
        tl.add(i * 1_000, i * 1_000 + 100); // all within [0, 10M)
    }

    assert_eq!(tl.ranges(), &[(0i64, 10_000_000)]);
}

#[test]
fn timeline_bucket_mode_many_buckets() {
    let mut tl = Timeline::<1_000_000>::new(); // 1-second buckets
    for i in 0..1_000 {
        tl.add((i * 1_000_000) as i64 + 500_000, (i * 1_000_000) as i64 + 600_000);
    }

    // Adjacent buckets merge into one continuous range
    assert_eq!(tl.ranges(), &[(0i64, 1_000_000_000)]);
}

#[test]
fn timeline_bucket_mode_from_range() {
    let tl = Timeline::<10_000_000>::from_range(3_000_000, 5_000_000);
    assert_eq!(tl.ranges(), &[(0i64, 10_000_000)]);
}

#[test]
fn timeline_bucket_mode_from_range_invalid() {
    let tl = Timeline::<10_000_000>::from_range(5_000_000, 3_000_000);
    assert!(tl.is_empty());
}
