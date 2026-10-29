use crate as pco_pack;
use chrono::{DateTime, Duration, Utc};
use pco_pack::{PcoPack, Resolution, Timeline};

fn bucketed(points_secs: &[i64]) -> Timeline {
    let mut timeline = Timeline::default();
    for &p in points_secs {
        timeline.add(10, p * 1_000_000);
    }
    timeline
}

#[test]
fn merge_preserves_an_incoming_range_spanning_multiple_buckets() {
    let mut destination = bucketed(&[50]);
    let incoming = bucketed(&[10, 20]);
    assert_eq!(incoming.ranges(), &[(10_000_000i64, 30_000_000)]);

    destination.merge(&incoming);

    assert_eq!(destination.ranges(), &[(10_000_000i64, 30_000_000), (50_000_000, 60_000_000)]);
    assert!(destination.contains(25_000_000));
}

#[test]
fn bucketed_merge_is_commutative() {
    let a = bucketed(&[50]);
    let b = bucketed(&[10, 20]);
    let mut a_then_b = a.clone();
    let mut b_then_a = b.clone();

    a_then_b.merge(&b);
    b_then_a.merge(&a);

    assert_eq!(a_then_b.ranges(), b_then_a.ranges());
}

#[test]
fn merge_preserves_multiple_incoming_coalesced_ranges() {
    let mut destination = bucketed(&[100]);
    let incoming = bucketed(&[10, 20, 50, 60]);

    destination.merge(&incoming);

    assert_eq!(
        destination.ranges(),
        &[(10_000_000i64, 30_000_000), (50_000_000, 70_000_000), (100_000_000, 110_000_000)]
    );
}

#[derive(Debug, PartialEq, PcoPack)]
struct Row {
    coverage: Timeline,
}

#[test]
fn merged_bucket_coverage_survives_serialization() {
    let mut coverage = bucketed(&[50]);
    coverage.merge(&bucketed(&[10, 20]));

    let bytes = Row::serialize(vec![Row { coverage }]).unwrap();
    let rows = Row::deserialize(&bytes).unwrap();

    assert_eq!(rows[0].coverage.ranges(), &[(10_000_000i64, 30_000_000), (50_000_000, 60_000_000)]);
    assert!(rows[0].coverage.contains(25_000_000));
}

#[test]
fn merge_into_empty_destination_preserves_all_buckets() {
    let mut destination = Timeline::default();
    let incoming = bucketed(&[10, 20]);
    destination.merge(&incoming);
    assert_eq!(destination.ranges(), incoming.ranges());
}

#[test]
fn merge_empty_source_leaves_destination_unchanged() {
    let mut destination = bucketed(&[10, 20]);
    destination.merge(&Timeline::default());
    assert_eq!(destination.ranges(), &[(10_000_000i64, 30_000_000)]);
}

#[test]
fn exact_mode_preserves_full_ranges() {
    let mut destination = Timeline::from_range(50, 60);
    destination.merge(&Timeline::from_range(10, 30));
    assert_eq!(destination.ranges(), &[(10i64, 30), (50, 60)]);
}

#[test]
fn single_bucket_merge_still_merges_adjacent_buckets() {
    let mut destination = bucketed(&[10]);
    destination.merge(&bucketed(&[20]));
    assert_eq!(destination.ranges(), &[(10_000_000i64, 30_000_000)]);
}

#[test]
fn merge_keeps_source_bucket_widths() {
    // A coarser destination never re-buckets finer incoming coverage, and vice versa.
    let mut destination = Timeline::default();
    destination.add(10, 5_000_000); // [0, 10M)

    let mut incoming = Timeline::default();
    incoming.add(1, 12_000_000); // [12M, 13M)

    destination.merge(&incoming);
    assert_eq!(destination.ranges(), &[(0i64, 10_000_000), (12_000_000, 13_000_000)]);

    let mut destination = Timeline::default();
    destination.add(1, 5_000_000); // [5M, 6M)
    let mut incoming = Timeline::default();
    incoming.add(10, 3_000_000); // [0, 10M)
    destination.merge(&incoming);
    assert_eq!(destination.ranges(), &[(0i64, 10_000_000)]);
}

#[test]
fn merge_preserves_sub_second_coverage() {
    let mut destination = Timeline::default();
    destination.add(10, 5_000_000); // [0, 10M)
    let mut incoming = Timeline::default();
    incoming.add(Duration::milliseconds(500), 10_000_000); // [10M, 10.5M), adjacent
    destination.merge(&incoming);
    assert_eq!(destination.ranges(), &[(0i64, 10_500_000)]);
}

#[test]
fn resolution_spellings_agree() {
    assert_eq!(Resolution::seconds(10), Resolution::from(10u32));
    assert_eq!(Resolution::seconds(10), Resolution::from(Duration::seconds(10)));
    assert_eq!(Resolution::exact(), Resolution::from(0u32));
    assert_eq!(Resolution::exact(), Resolution::from(Duration::zero()));
}

#[test]
fn constructors_bucket_and_keep_exact() {
    let tl = Timeline::new(10, 3_000_000);
    assert_eq!(tl.ranges(), &[(0i64, 10_000_000)]);

    let dt = DateTime::<Utc>::from_timestamp(1_767_225_600, 500_000_000).unwrap();
    let exact = Timeline::new_time(Resolution::exact(), dt);
    assert_eq!(exact.ranges(), &[(1_767_225_600_500_000i64, 1_767_225_600_500_000)]);

    let bucketed = Timeline::new_time(10, dt);
    assert_eq!(bucketed.ranges(), &[(1_767_225_600_000_000i64, 1_767_225_610_000_000)]);
}
