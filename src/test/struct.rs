use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct CompactRecord {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct ExpandedRecord {
    id: i64,
    name: String,
    score: f32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct Example {
    id: i64,
    name: String,
    score: f32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct Address {
    street: String,
    city: String,
    zip: u32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct Person {
    name: String,
    age: i32,
    address: Address,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum OldStatus {
    #[default]
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum NewStatus {
    #[default]
    Active,
    Inactive,
    Pending,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct ClickPayload {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
struct ScrollPayload {
    dx: i32,
    dy: i32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum OldEvent {
    #[default]
    Unknown,
    Click(ClickPayload),
    Resize,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum NewEvent {
    #[default]
    Unknown,
    Click(ClickPayload),
    Resize,
    Scroll(ScrollPayload),
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct OldMessage {
    id: i64,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct NewMessage {
    id: i64,
    text: String,
    priority: i32,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum OldAction {
    #[default]
    Create,
    Update,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum NewAction {
    #[default]
    Create,
    Update,
    Delete,
}

fn pco_pack_roundtrip<T: PcoPack>(data: Vec<T>) -> Vec<T> {
    let bytes = T::serialize(data).unwrap();
    T::deserialize(&bytes).unwrap()
}

fn pco_pack_cross_schema_read<A: PcoPack, B: PcoPack>(data: Vec<A>) -> Vec<B> {
    let bytes = A::serialize(data).unwrap();
    let compressed = B::from_bytes(&bytes).unwrap();
    <B as PcoPack>::read(compressed).unwrap()
}

#[test]
fn derive_struct_roundtrip() {
    let data: Vec<Example> = vec![
        Example { id: 1, name: "alice".into(), score: 95.5 },
        Example { id: 2, name: "bob".into(), score: 87.3 },
        Example { id: 3, name: "charlie".into(), score: 0.0 },
    ];
    let result = pco_pack_roundtrip(data.clone());
    assert_eq!(data, result);
}

#[test]
fn derive_struct_empty() {
    let bytes = Example::serialize(Vec::new()).unwrap();
    let result = Example::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn derive_struct_filter() {
    let data: Vec<Example> = vec![
        Example { id: 1, name: "alice".into(), score: 95.5 },
        Example { id: 2, name: "bob".into(), score: 87.3 },
        Example { id: 3, name: "charlie".into(), score: 0.0 },
    ];

    let bytes = Example::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"id": 1});
    let result = Example::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 1);
}

#[test]
fn struct_field_added_cross_schema() {
    let data: Vec<CompactRecord> =
        vec![CompactRecord { id: 1, name: "alice".into() }, CompactRecord { id: 2, name: "bob".into() }];
    let result: Vec<ExpandedRecord> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].name, "alice");
    assert_eq!(result[0].score, 0.0);
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].name, "bob");
    assert_eq!(result[1].score, 0.0);
}

#[test]
fn struct_field_removed_cross_schema() {
    let data: Vec<ExpandedRecord> = vec![
        ExpandedRecord { id: 10, name: "charlie".into(), score: 42.5 },
        ExpandedRecord { id: 20, name: "diana".into(), score: 99.9 },
    ];
    let result: Vec<CompactRecord> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0].id, 10);
    assert_eq!(result[0].name, "charlie");
    assert_eq!(result[1].id, 20);
    assert_eq!(result[1].name, "diana");
}

#[test]
fn struct_field_order_independent_roundtrip() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct OrderA {
        a: i64,
        b: String,
    }
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct OrderB {
        b: String,
        a: i64,
    }

    let data_a: Vec<OrderA> = vec![OrderA { a: 1, b: "one".into() }, OrderA { a: 2, b: "two".into() }];
    let result_a = pco_pack_roundtrip(data_a.clone());
    assert_eq!(data_a, result_a);

    let data_b: Vec<OrderB> = vec![OrderB { b: "one".into(), a: 1 }, OrderB { b: "two".into(), a: 2 }];
    let result_b = pco_pack_roundtrip(data_b.clone());
    assert_eq!(data_b, result_b);
}

#[test]
fn struct_middle_field_removed_cross_schema() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct FullRecord {
        id: i64,
        name: String,
        score: f32,
    }
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct CompactMiddle {
        id: i64,
        score: f32,
    }

    let data: Vec<FullRecord> = vec![
        FullRecord { id: 10, name: "charlie".into(), score: 42.5 },
        FullRecord { id: 20, name: "diana".into(), score: 99.9 },
    ];
    let result: Vec<CompactMiddle> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0].id, 10);
    assert_eq!(result[0].score, 42.5);
    assert_eq!(result[1].id, 20);
    assert_eq!(result[1].score, 99.9);
}

#[test]
fn struct_middle_field_added_cross_schema() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct OldMid {
        id: i64,
        name: String,
    }
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    struct NewMid {
        id: i64,
        title: String,
        name: String,
    }

    let data: Vec<OldMid> = vec![OldMid { id: 10, name: "charlie".into() }, OldMid { id: 20, name: "diana".into() }];
    let result: Vec<NewMid> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0].id, 10);
    assert_eq!(result[0].title, "");
    assert_eq!(result[0].name, "charlie");
    assert_eq!(result[1].id, 20);
    assert_eq!(result[1].title, "");
    assert_eq!(result[1].name, "diana");
}

#[test]
fn enum_variant_added_cross_schema() {
    let data: Vec<OldStatus> = vec![OldStatus::Active, OldStatus::Inactive, OldStatus::Pending, OldStatus::Active];
    let result: Vec<NewStatus> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0], NewStatus::Active);
    assert_eq!(result[1], NewStatus::Inactive);
    assert_eq!(result[2], NewStatus::Pending);
    assert_eq!(result[3], NewStatus::Active);
}

#[test]
fn enum_tuple_variant_added_cross_schema() {
    let data: Vec<OldEvent> = vec![
        OldEvent::Click(ClickPayload { x: 10, y: 20 }),
        OldEvent::Resize,
        OldEvent::Click(ClickPayload { x: 100, y: 200 }),
    ];
    let result: Vec<NewEvent> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0], NewEvent::Click(ClickPayload { x: 10, y: 20 }));
    assert_eq!(result[1], NewEvent::Resize);
    assert_eq!(result[2], NewEvent::Click(ClickPayload { x: 100, y: 200 }));
}

#[test]
fn struct_field_added_to_message_cross_schema() {
    let data: Vec<OldMessage> =
        vec![OldMessage { id: 1, text: "hello".into() }, OldMessage { id: 2, text: "world".into() }];
    let result: Vec<NewMessage> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].text, "hello");
    assert_eq!(result[0].priority, 0);
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].text, "world");
    assert_eq!(result[1].priority, 0);
}

#[test]
fn enum_unit_variant_added_cross_schema() {
    let data: Vec<OldAction> = vec![OldAction::Create, OldAction::Update, OldAction::Create];
    let result: Vec<NewAction> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0], NewAction::Create);
    assert_eq!(result[1], NewAction::Update);
    assert_eq!(result[2], NewAction::Create);
}

#[test]
fn struct_field_added_roundtrip() {
    let data: Vec<CompactRecord> =
        vec![CompactRecord { id: 1, name: "alice".into() }, CompactRecord { id: 2, name: "bob".into() }];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].name, "alice");
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].name, "bob");
}

#[test]
fn struct_field_removed_roundtrip() {
    let data: Vec<ExpandedRecord> = vec![
        ExpandedRecord { id: 10, name: "charlie".into(), score: 42.5 },
        ExpandedRecord { id: 20, name: "diana".into(), score: 99.9 },
    ];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0].id, 10);
    assert_eq!(result[0].name, "charlie");
    assert_eq!(result[0].score, 42.5);
    assert_eq!(result[1].id, 20);
    assert_eq!(result[1].name, "diana");
    assert_eq!(result[1].score, 99.9);
}

#[test]
fn enum_variant_added_roundtrip() {
    let data: Vec<OldStatus> = vec![OldStatus::Active, OldStatus::Inactive, OldStatus::Pending, OldStatus::Active];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0], OldStatus::Active);
    assert_eq!(result[1], OldStatus::Inactive);
    assert_eq!(result[2], OldStatus::Pending);
    assert_eq!(result[3], OldStatus::Active);
}

#[test]
fn enum_tuple_variant_added_roundtrip() {
    let data: Vec<OldEvent> = vec![
        OldEvent::Click(ClickPayload { x: 10, y: 20 }),
        OldEvent::Resize,
        OldEvent::Click(ClickPayload { x: 100, y: 200 }),
    ];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0], OldEvent::Click(ClickPayload { x: 10, y: 20 }));
    assert_eq!(result[1], OldEvent::Resize);
    assert_eq!(result[2], OldEvent::Click(ClickPayload { x: 100, y: 200 }));
}

#[test]
fn struct_field_added_to_message_roundtrip() {
    let data: Vec<OldMessage> =
        vec![OldMessage { id: 1, text: "hello".into() }, OldMessage { id: 2, text: "world".into() }];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].text, "hello");
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].text, "world");
}

#[test]
fn enum_unit_variant_added_roundtrip() {
    let data: Vec<OldAction> = vec![OldAction::Create, OldAction::Update, OldAction::Create];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0], OldAction::Create);
    assert_eq!(result[1], OldAction::Update);
    assert_eq!(result[2], OldAction::Create);
}

#[test]
fn struct_nested_roundtrip() {
    let data: Vec<Person> = vec![
        Person {
            name: "Alice".into(),
            age: 30,
            address: Address { street: "123 Main St".into(), city: "Springfield".into(), zip: 62704 },
        },
        Person {
            name: "Bob".into(),
            age: 25,
            address: Address { street: "456 Oak Ave".into(), city: "Shelbyville".into(), zip: 62565 },
        },
        Person {
            name: "Charlie".into(),
            age: 35,
            address: Address { street: "789 Elm Blvd".into(), city: "Capital City".into(), zip: 62701 },
        },
    ];

    let result = pco_pack_roundtrip(data.clone());
    assert_eq!(data, result);
}

#[test]
fn struct_nested_empty() {
    let bytes = Person::serialize(Vec::new()).unwrap();
    let result = Person::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn struct_nested_filter() {
    let data: Vec<Person> = vec![
        Person {
            name: "Alice".into(),
            age: 30,
            address: Address { street: "123 Main St".into(), city: "Springfield".into(), zip: 62704 },
        },
        Person {
            name: "Bob".into(),
            age: 25,
            address: Address { street: "456 Oak Ave".into(), city: "Shelbyville".into(), zip: 62565 },
        },
    ];

    let bytes = Person::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"age": 30});
    let result = Person::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Alice");
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct OldPoint2D {
    id: i64,
    coords: (i64, i64),
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct NewPoint3D {
    id: i64,
    coords: (i64, i64, i64),
}

#[test]
fn tuple_field_added_cross_schema() {
    let data: Vec<OldPoint2D> = vec![
        OldPoint2D { id: 1, coords: (10, 20) },
        OldPoint2D { id: 2, coords: (100, 200) },
        OldPoint2D { id: 3, coords: (5, 15) },
    ];
    let result: Vec<NewPoint3D> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].coords.0, 10);
    assert_eq!(result[0].coords.1, 20);
    assert_eq!(result[0].coords.2, 0);
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].coords.0, 100);
    assert_eq!(result[1].coords.1, 200);
    assert_eq!(result[1].coords.2, 0);
    assert_eq!(result[2].id, 3);
    assert_eq!(result[2].coords.0, 5);
    assert_eq!(result[2].coords.1, 15);
    assert_eq!(result[2].coords.2, 0);
}

#[test]
fn tuple_field_added_roundtrip() {
    let data: Vec<NewPoint3D> =
        vec![NewPoint3D { id: 1, coords: (10, 20, 30) }, NewPoint3D { id: 2, coords: (100, 200, 300) }];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].coords, (10, 20, 30));
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].coords, (100, 200, 300));
}

#[test]
fn tuple_field_removed_roundtrip() {
    let data: Vec<OldPoint2D> = vec![OldPoint2D { id: 1, coords: (10, 20) }, OldPoint2D { id: 2, coords: (100, 200) }];
    let result = pco_pack_roundtrip(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].coords, (10, 20));
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].coords, (100, 200));
}

#[test]
fn tuple_field_removed_cross_schema() {
    let data: Vec<NewPoint3D> =
        vec![NewPoint3D { id: 1, coords: (10, 20, 30) }, NewPoint3D { id: 2, coords: (100, 200, 300) }];
    let result: Vec<OldPoint2D> = pco_pack_cross_schema_read(data);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].coords, (10, 20));
    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].coords, (100, 200));
}
