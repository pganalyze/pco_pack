use pco_pack::{PcoPack, Timeline};

fn bucketed(points: &[i64]) -> Timeline<10> {
    let mut timeline = Timeline::new();
    for &point in points {
        timeline.add(point, point);
    }
    timeline
}

#[test]
fn merge_preserves_an_incoming_range_spanning_multiple_buckets() {
    let mut destination = bucketed(&[50]);
    let incoming = bucketed(&[10, 20]);
    assert_eq!(incoming.ranges(), &[(10, 30)]);

    destination.merge(&incoming);

    assert_eq!(destination.ranges(), &[(10, 30), (50, 60)]);
    assert!(destination.contains(25));
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

    assert_eq!(destination.ranges(), &[(10, 30), (50, 70), (100, 110)]);
}

#[derive(Debug, PartialEq, PcoPack)]
struct Row {
    coverage: Timeline<10>,
}

#[test]
fn merged_bucket_coverage_survives_serialization() {
    let mut coverage = bucketed(&[50]);
    coverage.merge(&bucketed(&[10, 20]));

    let bytes = Row::serialize(vec![Row { coverage }]).unwrap();
    let rows = Row::deserialize(&bytes).unwrap();

    assert_eq!(rows[0].coverage.ranges(), &[(10, 30), (50, 60)]);
    assert!(rows[0].coverage.contains(25));
}

#[test]
fn merge_into_empty_destination_preserves_all_buckets() {
    let mut destination = Timeline::<10>::new();
    let incoming = bucketed(&[10, 20]);
    destination.merge(&incoming);
    assert_eq!(destination.ranges(), incoming.ranges());
}

#[test]
fn merge_empty_source_leaves_destination_unchanged() {
    let mut destination = bucketed(&[10, 20]);
    destination.merge(&Timeline::new());
    assert_eq!(destination.ranges(), &[(10, 30)]);
}

#[test]
fn exact_mode_preserves_full_ranges() {
    let mut destination = Timeline::<0>::from_range(50, 60);
    destination.merge(&Timeline::from_range(10, 30));
    assert_eq!(destination.ranges(), &[(10, 30), (50, 60)]);
}

#[test]
fn single_bucket_merge_still_merges_adjacent_buckets() {
    let mut destination = bucketed(&[10]);
    destination.merge(&bucketed(&[20]));
    assert_eq!(destination.ranges(), &[(10, 30)]);
}
