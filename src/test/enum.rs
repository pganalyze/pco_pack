use crate as pco_pack;
use crate::{PcoPack, fold};

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
enum Status {
    #[default]
    Active,
    Inactive,
    Pending,
}

#[test]
fn enum_filter_type_error_includes_type() {
    let data = vec![Status::Active, Status::Inactive];
    let bytes = Status::serialize(data).unwrap();

    let result = Status::filter_bytes(&bytes, serde_json::json!({"": "nope"}), &[]);
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Expected i64 discriminant value for enum filter, got string"), "error message: {err}");

    let result = Status::filter_bytes(&bytes, serde_json::json!({"": {"start": true, "end": 2}}), &[]);
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Range start must be an integer, got boolean"), "error message: {err}");
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct Click {
    x: i32,
    y: i32,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct KeyPress {
    key: String,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
enum Event {
    #[default]
    Unknown,
    Click(Click),
    KeyPress(KeyPress),
    Resize,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
enum Priority {
    #[default]
    Low,
    Medium(i32),
    High,
    Critical(i32),
}

#[test]
fn enum_unit_only_roundtrip() {
    let data: Vec<Status> = vec![Status::Active, Status::Inactive, Status::Pending, Status::Active, Status::Active];

    let bytes = Status::serialize(data.clone()).unwrap();
    let result = Status::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn enum_chunk_envelope_roundtrip() {
    let data: Vec<Event> = (0..100)
        .map(|i| match i % 4 {
            0 => Event::Click(Click { x: i, y: i * 2 }),
            1 => Event::Resize,
            2 => Event::Unknown,
            _ => Event::KeyPress(KeyPress { key: format!("key_{i}") }),
        })
        .collect();

    let chunks = Event::write(data.clone()).unwrap();
    let bytes = Event::to_bytes(&chunks).unwrap();
    let decoded = Event::from_bytes(&bytes).unwrap();
    assert_eq!(chunks, decoded);
}

#[test]
fn enum_mixed_variants_roundtrip() {
    let data: Vec<Event> = vec![
        Event::Click(Click { x: 10, y: 20 }),
        Event::KeyPress(KeyPress { key: "hello".into() }),
        Event::Resize,
        Event::Click(Click { x: 100, y: 200 }),
        Event::KeyPress(KeyPress { key: "world".into() }),
        Event::Resize,
    ];

    let bytes = Event::serialize(data.clone()).unwrap();
    let result = Event::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn enum_empty() {
    let data: Vec<Status> = vec![];
    let bytes = Status::serialize(data).unwrap();
    let result = Status::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn enum_filter_discriminant_exact_match() {
    let data: Vec<Status> = vec![Status::Active, Status::Inactive, Status::Pending, Status::Active, Status::Inactive];

    let bytes = Status::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 0});
    let result = Status::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![Status::Active, Status::Active]);
}

#[test]
fn enum_filter_discriminant_no_match() {
    let data: Vec<Status> = vec![Status::Active, Status::Inactive, Status::Pending];

    let bytes = Status::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 99});
    let result = Status::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn single_filter() {
    let data: Vec<Status> = vec![Status::Active, Status::Inactive, Status::Pending, Status::Active, Status::Inactive];

    let bytes = Status::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 0});
    let result = Status::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![Status::Active, Status::Active]);
}

#[test]
fn filter_no_match() {
    let data: Vec<Status> = vec![Status::Active, Status::Inactive, Status::Pending];

    let bytes = Status::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 99});
    let result = Status::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filter_mixed_variants() {
    let data: Vec<Event> = vec![
        Event::Click(Click { x: 10, y: 20 }),
        Event::KeyPress(KeyPress { key: "hello".into() }),
        Event::Resize,
        Event::Click(Click { x: 100, y: 200 }),
        Event::Resize,
    ];

    let bytes = Event::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 1});
    let result = Event::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![Event::Click(Click { x: 10, y: 20 }), Event::Click(Click { x: 100, y: 200 })]);
}

#[derive(Clone, Debug, PartialEq, PcoPack)]
struct Record {
    id: i64,
    status: Status,
    label: String,
}

#[test]
fn struct_with_enum_field_roundtrip() {
    let data = vec![
        Record { id: 1, status: Status::Active, label: "alice".into() },
        Record { id: 2, status: Status::Inactive, label: "bob".into() },
        Record { id: 3, status: Status::Pending, label: "charlie".into() },
    ];

    let bytes = Record::serialize(data.clone()).unwrap();
    let result = Record::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn struct_with_enum_field_filter_by_status() {
    let data = vec![
        Record { id: 1, status: Status::Active, label: "alice".into() },
        Record { id: 2, status: Status::Inactive, label: "bob".into() },
        Record { id: 3, status: Status::Pending, label: "charlie".into() },
        Record { id: 4, status: Status::Active, label: "dave".into() },
    ];

    let bytes = Record::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"status": 0});
    let result = Record::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].status, Status::Active);
    assert_eq!(result[1].status, Status::Active);
}

#[test]
fn enum_multiple_tuple_variants_roundtrip() {
    let data: Vec<Priority> = vec![
        Priority::Low,
        Priority::Medium(1_i32),
        Priority::High,
        Priority::Critical(100_i32),
        Priority::Medium(2_i32),
        Priority::Low,
    ];

    let bytes = Priority::serialize(data.clone()).unwrap();
    let result = Priority::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn enum_multiple_tuple_variants_filter() {
    let data: Vec<Priority> = vec![
        Priority::Low,
        Priority::Medium(1_i32),
        Priority::High,
        Priority::Critical(100_i32),
        Priority::Medium(2_i32),
    ];

    let bytes = Priority::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 1});
    let result = Priority::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![Priority::Medium(1_i32), Priority::Medium(2_i32)]);
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
enum StatusWithLargeDiscriminant {
    #[default]
    Active = 10,
    Inactive = 20,
    Pending = 30,
}

#[test]
fn enum_filter_i64_discriminant_exact_match() {
    let data: Vec<StatusWithLargeDiscriminant> = vec![
        StatusWithLargeDiscriminant::Active,
        StatusWithLargeDiscriminant::Inactive,
        StatusWithLargeDiscriminant::Pending,
        StatusWithLargeDiscriminant::Active,
        StatusWithLargeDiscriminant::Inactive,
    ];

    let bytes = StatusWithLargeDiscriminant::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 10});
    let result = StatusWithLargeDiscriminant::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![StatusWithLargeDiscriminant::Active, StatusWithLargeDiscriminant::Active,]);
}

#[test]
fn enum_filter_i64_discriminant_no_match() {
    let data: Vec<StatusWithLargeDiscriminant> = vec![
        StatusWithLargeDiscriminant::Active,
        StatusWithLargeDiscriminant::Inactive,
        StatusWithLargeDiscriminant::Pending,
    ];

    let bytes = StatusWithLargeDiscriminant::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 99});
    let result = StatusWithLargeDiscriminant::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filter_i64_discriminant() {
    let data: Vec<StatusWithLargeDiscriminant> = vec![
        StatusWithLargeDiscriminant::Active,
        StatusWithLargeDiscriminant::Inactive,
        StatusWithLargeDiscriminant::Pending,
        StatusWithLargeDiscriminant::Active,
    ];

    let bytes = StatusWithLargeDiscriminant::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 20});
    let result = StatusWithLargeDiscriminant::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![StatusWithLargeDiscriminant::Inactive]);
}

#[test]
fn enum_all_same_variant() {
    let data: Vec<Status> = vec![Status::Active, Status::Active, Status::Active];

    let bytes = Status::serialize(data.clone()).unwrap();
    let result = Status::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn enum_single_row() {
    let data: Vec<Event> = vec![Event::Resize];

    let bytes = Event::serialize(data.clone()).unwrap();
    let result = Event::deserialize(&bytes).unwrap();
    assert_eq!(result, vec![Event::Resize]);
}

#[test]
fn enum_non_contiguous_filter_tuple_variants() {
    let data: Vec<Event> = vec![
        Event::Click(Click { x: 1, y: 2 }),
        Event::KeyPress(KeyPress { key: "a".into() }),
        Event::Click(Click { x: 3, y: 4 }),
        Event::Resize,
        Event::Click(Click { x: 5, y: 6 }),
    ];

    let bytes = Event::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 1});
    let result = Event::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(
        result,
        vec![
            Event::Click(Click { x: 1, y: 2 }),
            Event::Click(Click { x: 3, y: 4 }),
            Event::Click(Click { x: 5, y: 6 }),
        ]
    );
}

#[test]
fn enum_non_contiguous_filter_multiple_tuple_variants() {
    let data: Vec<Priority> = vec![
        Priority::Low,
        Priority::Medium(10_i32),
        Priority::Critical(99_i32),
        Priority::High,
        Priority::Medium(20_i32),
        Priority::Low,
        Priority::Critical(88_i32),
        Priority::Medium(30_i32),
    ];

    let bytes = Priority::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": 1});
    let result = Priority::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![Priority::Medium(10_i32), Priority::Medium(20_i32), Priority::Medium(30_i32)]);
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct VecEvent {
    field: Vec<Event>,
}

/// A zero-element tag column writes a zero-chunk pco block; the reader must
/// consume it so the variant columns after it decode.
#[test]
fn vec_event_all_empty_roundtrip() {
    let data: Vec<VecEvent> = vec![VecEvent { field: vec![] }, VecEvent { field: vec![] }];
    let bytes = VecEvent::serialize(data.clone()).unwrap();
    let result = VecEvent::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_event_empty_tail_roundtrip() {
    let data: Vec<VecEvent> = vec![VecEvent { field: vec![Event::Resize] }, VecEvent { field: vec![] }];
    let bytes = VecEvent::serialize(data.clone()).unwrap();
    let result = VecEvent::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn enum_filter_inclusion_non_contiguous() {
    let data: Vec<Priority> = vec![
        Priority::Low,
        Priority::Medium(10_i32),
        Priority::High,
        Priority::Critical(99_i32),
        Priority::Medium(20_i32),
    ];

    let bytes = Priority::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"": [1, 3]});
    let result = Priority::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![Priority::Medium(10_i32), Priority::Critical(99_i32), Priority::Medium(20_i32)]);
}

fn priority_data() -> Vec<Priority> {
    (0..200)
        .map(|i| match i % 4 {
            0 => Priority::Low,
            1 => Priority::Medium(i),
            2 => Priority::High,
            _ => Priority::Critical(i * 2),
        })
        .collect()
}

/// A scalar-payload variant folds through an annotated binding: the closure receives
/// the payload value directly, and the fold streams payload slots without decoding
/// the discriminant column.
#[test]
fn enum_scalar_binding_fold() {
    let data = priority_data();
    let chunks = Priority::write(data.clone()).unwrap();

    let mut seen = Vec::new();
    Priority::for_each(&chunks, serde_json::json!({}), &[], fold!(Priority, |Medium(value): i32| seen.push(value)))
        .unwrap();
    let truth: Vec<i32> = data
        .iter()
        .filter_map(|p| match p {
            Priority::Medium(v) => Some(*v),
            _ => None,
        })
        .collect();
    assert_eq!(seen, truth);
}

/// A query pinning exactly the folded variant also reaches the slot fast path: the
/// survivors are that variant's rows, so the discriminant never decodes.
#[test]
fn enum_scalar_binding_fold_pinned_query() {
    let chunks = Priority::write(priority_data()).unwrap();

    let mut fast = Vec::new();
    Priority::for_each(
        &chunks,
        serde_json::json!({"Medium": true}),
        &[],
        fold!(Priority, |Medium(value): i32| fast.push(value)),
    )
    .unwrap();
    let mut hydrating: Vec<i32> = Priority::filter(&chunks, serde_json::json!({"Medium": true}), &[])
        .unwrap()
        .iter()
        .map(|p| match p {
            Priority::Medium(v) => *v,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    fast.sort();
    hydrating.sort();
    assert_eq!(fast, hydrating);
}

/// A qualified binding spells the same fold as the bare name.
#[test]
fn enum_scalar_binding_fold_qualified() {
    let chunks = Priority::write(priority_data()).unwrap();

    let mut bare = 0i64;
    Priority::for_each(&chunks, serde_json::json!({}), &[], fold!(Priority, |Medium(value): i32| bare += value as i64))
        .unwrap();
    let mut qualified = 0i64;
    Priority::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Priority, |Priority::Medium(value): i32| qualified += value as i64),
    )
    .unwrap();
    assert_eq!(bare, qualified);
    assert!(bare > 0);
}

/// Scalar bindings mix with unit arms in one multi-arm pass; each payload arm reads
/// through its own slot cursor, which the debug cross-check ties to its delivered-row
/// count.
#[test]
fn enum_scalar_binding_fold_multi_arm() {
    let data = priority_data();
    let chunks = Priority::write(data.clone()).unwrap();

    let mut mediums = Vec::new();
    let mut criticals = Vec::new();
    let mut low = 0usize;
    let mut high = 0usize;
    Priority::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(
            Priority,
            |Medium(value): i32| mediums.push(value),
            |Low| low += 1,
            |High| high += 1,
            |Critical(value): i32| criticals.push(value),
        ),
    )
    .unwrap();

    mediums.sort();
    criticals.sort();
    let medium_truth: Vec<i32> = data
        .iter()
        .filter_map(|p| match p {
            Priority::Medium(v) => Some(*v),
            _ => None,
        })
        .collect();
    let critical_truth: Vec<i32> = data
        .iter()
        .filter_map(|p| match p {
            Priority::Critical(v) => Some(*v),
            _ => None,
        })
        .collect();
    assert_eq!(mediums, medium_truth);
    assert_eq!(criticals, critical_truth);
    assert_eq!(low, data.iter().filter(|p| matches!(p, Priority::Low)).count());
    assert_eq!(high, data.iter().filter(|p| matches!(p, Priority::High)).count());
}

/// A binding annotated with the struct payload type hands the closure the whole
/// payload value instead of selected fields.
#[test]
fn enum_whole_payload_binding_fold() {
    let data: Vec<Event> = (0..60)
        .map(|i| match i % 3 {
            0 => Event::Click(Click { x: i, y: -i }),
            1 => Event::Resize,
            _ => Event::Unknown,
        })
        .collect();
    let chunks = Event::write(data.clone()).unwrap();

    let mut clicks = Vec::new();
    Event::for_each(
        &chunks,
        serde_json::json!({}),
        &[],
        fold!(Event, |Click(click): Click| clicks.push((click.x, click.y))),
    )
    .unwrap();
    let truth: Vec<(i32, i32)> = data
        .iter()
        .filter_map(|e| match e {
            Event::Click(c) => Some((c.x, c.y)),
            _ => None,
        })
        .collect();
    assert_eq!(clicks, truth);
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
enum WideSchema {
    #[default]
    Zero,
    Hidden = 7,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
enum NarrowSchema {
    #[default]
    Zero,
    One,
}

/// A discriminant outside the reader's variant table is corruption or an
/// incompatible schema, and errors instead of hydrating a silent default row.
#[test]
fn enum_unknown_discriminant_errors() {
    let chunks = WideSchema::write(vec![WideSchema::Zero, WideSchema::Hidden]).unwrap();
    let bytes = WideSchema::to_bytes(&chunks).unwrap();
    let mismatched = NarrowSchema::from_bytes(&bytes).unwrap();

    let err = NarrowSchema::read(mismatched).unwrap_err().to_string();
    assert!(err.contains("has no variant with discriminant 7"), "error message: {err}");
}

/// Block lengths inside an enum chunk are data: a truncated or length-corrupted
/// chunk must surface as a decode error, never an out-of-range panic.
#[test]
fn enum_truncated_chunk_errors() {
    let data: Vec<Event> = (0..30).map(|i| Event::Click(Click { x: i, y: i })).collect();

    let mut truncated = Event::write(data.clone()).unwrap();
    let mut raw = std::mem::take(&mut truncated[0]).into_vec();
    raw.truncate(9);
    truncated[0] = crate::serde_bytes::ByteBuf::from(raw);
    let err = Event::filter(&truncated, serde_json::json!({}), &[]).unwrap_err().to_string();
    assert!(err.contains("enum block"), "error message: {err}");

    let mut huge = Event::write(data).unwrap();
    let mut raw = std::mem::take(&mut huge[0]).into_vec();
    raw[..8].fill(0xFF);
    huge[0] = crate::serde_bytes::ByteBuf::from(raw);
    let err = Event::filter(&huge, serde_json::json!({}), &[]).unwrap_err().to_string();
    assert!(err.contains("enum block"), "error message: {err}");
}
