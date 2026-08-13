use crate as pco_pack;
use crate::PcoPack;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecI64 {
    values: Vec<i64>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecI8 {
    values: Vec<i8>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecU32 {
    values: Vec<u32>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecU8 {
    values: Vec<u8>,
}

#[test]
fn vec_i64_to_i8_coercion() {
    let original = vec![VecI64 { values: vec![42, 100, 127, 50] }];
    let bytes = VecI64::serialize(original).unwrap();
    let result = VecI8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, vec![VecI8 { values: vec![42i8, 100, 127, 50] }]);
}

#[test]
fn vec_u32_to_u8_coercion() {
    let original = vec![VecU32 { values: vec![10, 20, 30] }];
    let bytes = VecU32::serialize(original).unwrap();
    let result = VecU8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, vec![VecU8 { values: vec![10u8, 20, 30] }]);
}

#[test]
fn vec_i64_out_of_range_clamps() {
    let original = vec![VecI64 { values: vec![300, -200, 127] }];
    let bytes = VecI64::serialize(original).unwrap();
    let result = VecI8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    // 300 -> 127 (clamp to MAX), -200 -> -128 (clamp to MIN), 127 -> 127
    assert_eq!(result, vec![VecI8 { values: vec![127i8, -128, 127] }]);
}

#[test]
fn vec_multiple_rows_coercion() {
    let original = vec![VecI64 { values: vec![1, 2] }, VecI64 { values: vec![3, 4, 5] }];
    let bytes = VecI64::serialize(original).unwrap();
    let result = VecI8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, vec![VecI8 { values: vec![1i8, 2] }, VecI8 { values: vec![3, 4, 5] },]);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecOptI64 {
    values: Vec<Option<i64>>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecOptI8 {
    values: Vec<Option<i8>>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecOptF64 {
    values: Vec<Option<f64>>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct VecOptF32 {
    values: Vec<Option<f32>>,
}

#[test]
fn option_i64_to_i8_coercion() {
    let original = vec![VecOptI64 { values: vec![Some(42), None, Some(100)] }];
    let bytes = VecOptI64::serialize(original).unwrap();
    let result = VecOptI8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, vec![VecOptI8 { values: vec![Some(42i8), None, Some(100i8)] }]);
}

#[test]
fn option_f64_to_f32_coercion() {
    let original = vec![VecOptF64 { values: vec![Some(1.5), Some(2.5), None] }];
    let bytes = VecOptF64::serialize(original).unwrap();
    let result = VecOptF32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, vec![VecOptF32 { values: vec![Some(1.5f32), Some(2.5f32), None] }]);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct BTreeMapI64I64 {
    map: BTreeMap<i64, i64>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct BTreeMapI8I8 {
    map: BTreeMap<i8, i8>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct BTreeMapU64U64 {
    map: BTreeMap<u64, u64>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct BTreeMapU8U8 {
    map: BTreeMap<u8, u8>,
}

#[test]
fn btreemap_i64_to_i8_coercion() {
    let mut m1 = BTreeMap::new();
    m1.insert(1, 10);
    m1.insert(2, 20);
    let original = vec![BTreeMapI64I64 { map: m1 }];
    let bytes = BTreeMapI64I64::serialize(original).unwrap();
    let result = BTreeMapI8I8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].map.get(&1), Some(&10));
    assert_eq!(result[0].map.get(&2), Some(&20));
}

#[test]
fn btreemap_u64_to_u8_coercion() {
    let mut m = BTreeMap::new();
    m.insert(100, 200);
    let original = vec![BTreeMapU64U64 { map: m }];
    let bytes = BTreeMapU64U64::serialize(original).unwrap();
    let result = BTreeMapU8U8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result[0].map.get(&100), Some(&200));
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct HashMapI64I64 {
    map: HashMap<i64, i64>,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct HashMapI8I8 {
    map: HashMap<i8, i8>,
}

#[test]
fn hashmap_i64_to_i8_coercion() {
    let mut m1 = HashMap::new();
    m1.insert(1, 10);
    m1.insert(2, 20);
    let original = vec![HashMapI64I64 { map: m1 }];
    let bytes = HashMapI64I64::serialize(original).unwrap();
    let result = HashMapI8I8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result[0].map.get(&1), Some(&10));
    assert_eq!(result[0].map.get(&2), Some(&20));
}

#[test]
fn struct_i64_field_coerced_to_i8() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TestRecord {
        id: i64,
        score: f64,
        label: String,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TestRecordSmall {
        id: i8,
        score: f64,
        label: String,
    }

    let original = vec![
        TestRecord { id: 42, score: 3.14, label: "hello".into() },
        TestRecord { id: 100, score: 2.71, label: "world".into() },
    ];
    let bytes = TestRecord::serialize(original).unwrap();
    let results = TestRecordSmall::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, 42);
    assert_eq!(results[1].id, 100);
    assert_eq!(results[0].score, 3.14);
    assert_eq!(results[1].score, 2.71);
}

#[test]
fn struct_f64_field_coerced_to_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TestRecord {
        id: i64,
        score: f64,
        label: String,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct TestRecordSmall {
        id: i64,
        score: f32,
        label: String,
    }

    let original =
        vec![TestRecord { id: 1, score: 1.5, label: "a".into() }, TestRecord { id: 2, score: 2.5, label: "b".into() }];
    let bytes = TestRecord::serialize(original).unwrap();
    let results = TestRecordSmall::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].score, 1.5f32);
    assert_eq!(results[1].score, 2.5f32);
}

#[test]
fn vec_option_i64_to_vec_option_i8() {
    let original = vec![VecOptI64 { values: vec![Some(1), Some(2), None, Some(4)] }];
    let bytes = VecOptI64::serialize(original).unwrap();
    let result = VecOptI8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, vec![VecOptI8 { values: vec![Some(1i8), Some(2i8), None, Some(4i8)] }]);
}

#[test]
fn struct_vec_i64_field_coerced_to_vec_i8() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct VecRecord {
        id: i64,
        values: Vec<i64>,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct VecRecordSmall {
        id: i64,
        values: Vec<i8>,
    }

    let original = vec![VecRecord { id: 1, values: vec![10, 20, 30] }, VecRecord { id: 2, values: vec![40, 50] }];
    let bytes = VecRecord::serialize(original).unwrap();
    let results = VecRecordSmall::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].values, vec![10i8, 20, 30]);
    assert_eq!(results[1].values, vec![40i8, 50]);
}

#[test]
fn struct_option_f64_field_coerced_to_option_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct OptRecord {
        id: i64,
        score: Option<f64>,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct OptRecordSmall {
        id: i64,
        score: Option<f32>,
    }

    let original = vec![OptRecord { id: 1, score: Some(1.5) }, OptRecord { id: 2, score: None }];
    let bytes = OptRecord::serialize(original).unwrap();
    let results = OptRecordSmall::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].score, Some(1.5f32));
    assert_eq!(results[1].score, None);
}

#[test]
fn struct_btreemap_i64_field_coerced_to_btreemap_i8() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MapRecord {
        id: i64,
        tags: BTreeMap<i64, i64>,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct MapRecordSmall {
        id: i64,
        tags: BTreeMap<i8, i8>,
    }

    let mut tags1 = BTreeMap::new();
    tags1.insert(1, 10);
    tags1.insert(2, 20);
    let mut tags2 = BTreeMap::new();
    tags2.insert(3, 30);
    let original = vec![MapRecord { id: 1, tags: tags1 }, MapRecord { id: 2, tags: tags2 }];
    let bytes = MapRecord::serialize(original).unwrap();
    let results = MapRecordSmall::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].tags.get(&1), Some(&10));
    assert_eq!(results[0].tags.get(&2), Some(&20));
    assert_eq!(results[1].tags.get(&3), Some(&30));
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct TupleI64 {
    a: i64,
    b: i64,
}
#[derive(Debug, Clone, PartialEq, PcoPack)]
struct TupleI8 {
    a: i8,
    b: i8,
}

#[test]
fn tuple_i64_to_i8_coercion() {
    let original = vec![TupleI64 { a: 1, b: 2 }, TupleI64 { a: 3, b: 4 }];
    let bytes = TupleI64::serialize(original).unwrap();
    let result = TupleI8::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result, vec![TupleI8 { a: 1i8, b: 2i8 }, TupleI8 { a: 3i8, b: 4i8 }]);
}
