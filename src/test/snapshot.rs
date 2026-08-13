use crate as pco_pack;
use crate::{PcoPack, Timeline};
use std::collections::{BTreeMap, HashMap};

/// A struct exercising every field type supported by PcoPack.
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct SnapshotStruct {
    // Integer types
    pub i8_val: i8,
    pub i16_val: i16,
    pub i32_val: i32,
    pub i64_val: i64,
    pub u8_val: u8,
    pub u16_val: u16,
    pub u32_val: u32,
    pub u64_val: u64,

    // Float types
    pub f16_val: half::f16,
    pub f32_val: f32,
    pub f64_val: f64,

    // Boolean
    pub bool_val: bool,

    // String types
    pub string_val: String,
    pub smol_str_val: smol_str::SmolStr,

    // Bytes / UUID / JSON
    pub bytes_val: serde_bytes::ByteBuf,
    pub uuid_val: uuid::Uuid,
    pub json_val: serde_json::Value,

    // Datetime / Timeline
    pub datetime_val: chrono::DateTime<chrono::Utc>,
    pub timeline_val: Timeline<0>,

    // Option wrappers
    pub opt_i64: Option<i64>,
    pub opt_string: Option<String>,
    pub opt_bool: Option<bool>,

    // Vec collections
    pub vec_i32: Vec<i32>,
    pub vec_f64: Vec<f64>,
    pub vec_string: Vec<String>,
    pub vec_option_i32: Vec<Option<i32>>,

    // Map collections
    pub hashmap_str_i64: HashMap<String, i64>,
    pub btremap_str_f64: BTreeMap<String, f64>,

    // Tuple field
    pub tuple_val: (i32, String),

    // Nested struct
    pub nested: SnapshotNestedStruct,

    // Enum field
    pub enum_val: SnapshotEnum,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct SnapshotNestedStruct {
    pub x: i32,
    pub y: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
enum SnapshotEnum {
    #[default]
    None,
    Int(i32),
    Float(f64),
    Text(String),
}

fn make_struct_data() -> Vec<SnapshotStruct> {
    vec![
        SnapshotStruct {
            i8_val: -127,
            i16_val: 30_000,
            i32_val: -1_000_000,
            i64_val: 9_223_372_036_854_775_807_i64,
            u8_val: 255,
            u16_val: 65_535,
            u32_val: 4_294_967_295_u32,
            u64_val: 0xFFFFFFFFFFFFFFFF_u64,
            f16_val: half::f16::from_f32(1.5),
            f32_val: -3.14159,
            f64_val: 2.718281828459045,
            bool_val: true,
            string_val: "hello world".into(),
            smol_str_val: "smol text".into(),
            bytes_val: serde_bytes::ByteBuf::from(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            uuid_val: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            json_val: serde_json::json!({"key": "value", "num": 42}),
            datetime_val: chrono::DateTime::<chrono::Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
            timeline_val: {
                let mut tl = Timeline::<0>::new();
                tl.add(1_000_000, 5_000_000);
                tl.add(10_000_000, 20_000_000);
                tl
            },
            opt_i64: Some(-99),
            opt_string: None,
            opt_bool: Some(false),
            vec_i32: vec![1, 2, 3],
            vec_f64: vec![1.1, 2.2, -3.3],
            vec_string: vec!["a".into(), "b".into()],
            vec_option_i32: vec![Some(10), None, Some(30)],

            hashmap_str_i64: {
                let mut m = HashMap::new();
                m.insert("x".into(), 1);
                m.insert("y".into(), 2);
                m
            },
            btremap_str_f64: {
                let mut m = BTreeMap::new();
                m.insert("a".into(), 0.5);
                m.insert("b".into(), -1.5);
                m
            },
            tuple_val: (42, "tuple text".into()),
            nested: SnapshotNestedStruct { x: 7, y: 3.14, label: "nested".into() },
            enum_val: SnapshotEnum::Int(99),
        },
        SnapshotStruct {
            i8_val: 0,
            i16_val: -1,
            i32_val: 0,
            i64_val: 0,
            u8_val: 0,
            u16_val: 0,
            u32_val: 0,
            u64_val: 0,
            f16_val: half::f16::from_f32(-0.5),
            f32_val: 0.0,
            f64_val: -999.999,
            bool_val: false,
            string_val: "".into(),
            smol_str_val: "".into(),
            bytes_val: serde_bytes::ByteBuf::new(),
            uuid_val: uuid::Uuid::nil(),
            json_val: serde_json::json!(null),
            datetime_val: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
            timeline_val: Timeline::<0>::from_range(100, 200),
            opt_i64: None,
            opt_string: Some("present".into()),
            opt_bool: None,
            vec_i32: vec![],
            vec_f64: vec![0.0],
            vec_string: vec!["single".into()],
            vec_option_i32: vec![None],

            hashmap_str_i64: HashMap::new(),
            btremap_str_f64: BTreeMap::new(),
            tuple_val: (-1, "".into()),
            nested: SnapshotNestedStruct { x: 0, y: 0.0, label: "".into() },
            enum_val: SnapshotEnum::Float(1.23),
        },
    ]
}

fn make_enum_data() -> Vec<SnapshotEnum> {
    vec![
        SnapshotEnum::None,
        SnapshotEnum::Int(-42),
        SnapshotEnum::Float(3.141592653589793),
        SnapshotEnum::Text("enum text".into()),
        SnapshotEnum::Int(0),
    ]
}

fn snapshot_dir() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    std::path::PathBuf::from(manifest_dir).join("src/test/snapshot")
}

fn ensure_snapshot_dir() {
    std::fs::create_dir_all(snapshot_dir()).expect("failed to create snapshot dir");
}

fn read_snapshot(name: &str) -> Option<Vec<u8>> {
    let path = snapshot_dir().join(name);
    std::fs::read(&path).ok()
}

fn write_snapshot(name: &str, data: &[u8]) {
    ensure_snapshot_dir();
    let path = snapshot_dir().join(name);
    std::fs::write(&path, data).expect("failed to write snapshot");
}

fn snapshot_roundtrip<T>(name: &str, expected_data: Vec<T>)
where
    T: PcoPack + Clone + PartialEq + std::fmt::Debug,
{
    let serialized = T::serialize(expected_data.clone()).expect("serialization failed");

    match read_snapshot(name) {
        Some(existing_bytes) => {
            let deserialized = T::deserialize(&existing_bytes).expect("deserialization of snapshot failed");
            assert_eq!(
                expected_data, deserialized,
                "Snapshot '{}' mismatch: old binary blob no longer roundtrips to the same data",
                name
            );
        }
        None => {
            eprintln!("Writing new snapshot '{}'", name);
            write_snapshot(name, &serialized);
        }
    }
}

#[test]
fn struct_snapshot() {
    let data = make_struct_data();
    snapshot_roundtrip::<SnapshotStruct>("struct.bin", data);
}

#[test]
fn enum_snapshot() {
    let data = make_enum_data();
    snapshot_roundtrip::<SnapshotEnum>("enum.bin", data);
}
