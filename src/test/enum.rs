use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum Status {
    #[default]
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct Click {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct KeyPress {
    key: String,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum Event {
    #[default]
    Unknown,
    Click(Click),
    KeyPress(KeyPress),
    Resize,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
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

#[derive(Debug, Clone, PartialEq, PcoPack)]
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

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
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
