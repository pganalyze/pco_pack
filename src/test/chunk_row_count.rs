use crate as pco_pack;
use crate::{PcoPack, Timeline};
use chrono::{DateTime, Utc};

fn dt(us: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_micros(us).unwrap()
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
#[pco_pack(index = [device_id])]
struct TelemetryRow {
    device_id: i64,
    value: i64,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
#[pco_pack(index = [tag])]
struct TagOnlyRow {
    tag: i64,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct NoFieldRow {}

#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(timestamp = seen_at)]
struct TimelineRow {
    seen_at: Timeline,
    value: i64,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
enum Metric {
    #[default]
    Idle,
    Gauge(f64),
    Note(String),
}

fn telemetry_data() -> Vec<TelemetryRow> {
    let mut data: Vec<TelemetryRow> = (0..3).map(|i| TelemetryRow { device_id: 1, value: i }).collect();
    data.extend((0..2).map(|i| TelemetryRow { device_id: 2, value: 10 + i }));
    data
}

fn sorted(mut rows: Vec<TelemetryRow>) -> Vec<TelemetryRow> {
    rows.sort_by_key(|r| (r.device_id, r.value));
    rows
}

#[test]
fn written_chunk_exposes_row_count() {
    let data = telemetry_data();
    let chunks = TelemetryRow::write(data.clone()).unwrap();
    assert_eq!(chunks.len(), 2);
    let written: Vec<usize> = chunks.iter().map(|c| c.chunk_size as usize).collect();
    let expanded: Vec<usize> = chunks.iter().map(|c| c.clone().expand().unwrap().len()).collect();
    assert_eq!(written, expanded);
    assert_eq!(written.iter().sum::<usize>(), data.len());
    let roundtrip: Vec<TelemetryRow> = chunks.into_iter().flat_map(|c| c.expand().unwrap()).collect();
    assert_eq!(sorted(roundtrip), sorted(data));
}

#[test]
fn legacy_chunk_without_row_count_decodes_to_same_rows() {
    let data = telemetry_data();
    let chunks = TelemetryRow::write(data.clone()).unwrap();
    let mut legacy = chunks.clone();
    for c in legacy.iter_mut() {
        c.chunk_size = 0;
    }
    assert_eq!(sorted(<TelemetryRow as PcoPack>::read(legacy.clone()).unwrap()), sorted(data));

    // Filtered reads must agree with the written twin for one query and projection.
    let query = serde_json::json!({"value": 11});
    let written_hits = TelemetryRow::filter(&chunks, query.clone(), &["device_id"]).unwrap();
    let legacy_hits = TelemetryRow::filter(&legacy, query, &["device_id"]).unwrap();
    assert_eq!(written_hits, legacy_hits);
}

#[test]
fn index_only_struct_expands_to_one_row() {
    let data: Vec<TagOnlyRow> = (0..4).map(|i| TagOnlyRow { tag: i % 2 }).collect();
    let chunks = TagOnlyRow::write(data).unwrap();
    assert_eq!(chunks.len(), 2);
    for c in &chunks {
        assert_eq!(c.chunk_size, 1);
        assert_eq!(c.clone().expand().unwrap(), vec![TagOnlyRow { tag: c.tag }]);
    }
}

#[test]
fn field_less_struct_answers_one_default_row() {
    // A record with no fields at all keeps the payload-less answer of one row per
    // chunk: there is no per-row data to report more (or fewer) than once.
    let chunks = NoFieldRow::write(vec![NoFieldRow::default(); 3]).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].chunk_size, 1);
    assert_eq!(chunks[0].clone().expand().unwrap(), vec![NoFieldRow::default()]);
}

#[test]
fn index_only_projection_returns_one_row_per_chunk() {
    let data = telemetry_data();
    let chunks = TelemetryRow::write(data).unwrap();
    let result = TelemetryRow::filter(&chunks, serde_json::json!({}), &["device_id"]).unwrap();
    assert_eq!(result.len(), 2);
    let mut device_ids: Vec<i64> = result.iter().map(|r| r.device_id).collect();
    device_ids.sort_unstable();
    assert_eq!(device_ids, vec![1, 2]);
    assert!(result.iter().all(|r| r.value == 0));

    let one_group = TelemetryRow::filter(&chunks, serde_json::json!({"device_id": 2}), &["device_id"]).unwrap();
    assert_eq!(one_group.len(), 1);
    assert_eq!(one_group[0].device_id, 2);
}

#[test]
fn enum_chunk_exposes_row_count() {
    let data = vec![Metric::Idle, Metric::Gauge(1.0), Metric::Note("a".into()), Metric::Gauge(2.0)];
    let chunks = Metric::write(data.clone()).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].chunk_size as usize, data.len());
    assert_eq!(<Metric as PcoPack>::read(chunks.clone()).unwrap(), data);

    let mut legacy = chunks.clone();
    legacy[0].chunk_size = 0;
    assert_eq!(<Metric as PcoPack>::read(legacy).unwrap(), data);
}

#[test]
fn timeline_chunk_size_matches_merged_rows() {
    let data = vec![
        TimelineRow { seen_at: Timeline::from_range(dt(1_000_000), dt(2_000_000)), value: 10 },
        TimelineRow { seen_at: Timeline::from_range(dt(3_000_000), dt(4_000_000)), value: 10 },
        TimelineRow { seen_at: Timeline::from_range(dt(5_000_000), dt(6_000_000)), value: 20 },
    ];
    let chunks = TimelineRow::write(data).unwrap();
    assert_eq!(chunks.len(), 1);
    // The two `value: 10` rows merge into one, so the chunk holds 2 rows.
    assert_eq!(chunks[0].chunk_size, 2);
    let expanded = chunks[0].clone().expand().unwrap();
    assert_eq!(expanded.len(), 2);
    assert_eq!(expanded.iter().map(|r| r.value).collect::<Vec<_>>(), vec![10, 20]);

    let mut legacy = chunks[0].clone();
    legacy.chunk_size = 0;
    assert_eq!(legacy.expand().unwrap(), expanded);
}
