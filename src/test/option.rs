use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct OptI32 {
    field: Option<i32>,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct OptU64 {
    field: Option<u64>,
}

#[test]
fn option_i32_roundtrip() {
    let data: Vec<OptI32> = vec![
        OptI32 { field: Some(1) },
        OptI32 { field: None },
        OptI32 { field: Some(42) },
        OptI32 { field: None },
        OptI32 { field: Some(-100) },
        OptI32 { field: Some(0) },
    ];
    let bytes = OptI32::serialize(data.clone()).unwrap();
    let result = OptI32::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn option_all_none() {
    let data: Vec<OptU64> = vec![OptU64 { field: None }, OptU64 { field: None }, OptU64 { field: None }];
    let bytes = OptU64::serialize(data.clone()).unwrap();
    let result = OptU64::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn option_all_some() {
    let data: Vec<OptU64> = vec![OptU64 { field: Some(1) }, OptU64 { field: Some(2) }, OptU64 { field: Some(3) }];
    let bytes = OptU64::serialize(data.clone()).unwrap();
    let result = OptU64::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn empty_option_column() {
    let data: Vec<OptI32> = vec![];
    let bytes = OptI32::serialize(data).unwrap();
    let result = OptI32::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filtered_reader_option_i32() {
    let data: Vec<OptI32> = vec![
        OptI32 { field: Some(10) },
        OptI32 { field: None },
        OptI32 { field: Some(42) },
        OptI32 { field: Some(42) },
        OptI32 { field: None },
    ];
    let bytes = OptI32::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"field": 42});
    let result = OptI32::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result, vec![OptI32 { field: Some(42) }, OptI32 { field: Some(42) }]);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct InnerRecord {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct OuterRecord {
    label: String,
    inner: Option<InnerRecord>,
}

#[test]
fn option_filter_nested_basic() {
    let data: Vec<OuterRecord> = vec![
        OuterRecord { label: "a".into(), inner: Some(InnerRecord { id: 1, name: "one".into() }) },
        OuterRecord { label: "b".into(), inner: None },
        OuterRecord { label: "c".into(), inner: Some(InnerRecord { id: 2, name: "two".into() }) },
        OuterRecord { label: "d".into(), inner: Some(InnerRecord { id: 3, name: "three".into() }) },
        OuterRecord { label: "e".into(), inner: None },
    ];
    let bytes = OuterRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"inner.id": 2});
    let result = OuterRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].label, "c");
    assert_eq!(result[0].inner.as_ref().unwrap().id, 2);
}

#[test]
fn option_filter_nested_multiple_matches() {
    let data: Vec<OuterRecord> = vec![
        OuterRecord { label: "a".into(), inner: None },
        OuterRecord { label: "b".into(), inner: Some(InnerRecord { id: 5, name: "five".into() }) },
        OuterRecord { label: "c".into(), inner: Some(InnerRecord { id: 5, name: "five".into() }) },
        OuterRecord { label: "d".into(), inner: None },
        OuterRecord { label: "e".into(), inner: Some(InnerRecord { id: 7, name: "seven".into() }) },
    ];
    let bytes = OuterRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"inner.id": 5});
    let result = OuterRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].label, "b");
    assert_eq!(result[1].label, "c");
}

#[test]
fn option_filter_nested_string_field() {
    let data: Vec<OuterRecord> = vec![
        OuterRecord { label: "x".into(), inner: Some(InnerRecord { id: 1, name: "alpha".into() }) },
        OuterRecord { label: "y".into(), inner: None },
        OuterRecord { label: "z".into(), inner: Some(InnerRecord { id: 2, name: "beta".into() }) },
        OuterRecord { label: "w".into(), inner: Some(InnerRecord { id: 3, name: "alpha".into() }) },
    ];
    let bytes = OuterRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"inner.name": "alpha"});
    let result = OuterRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].label, "x");
    assert_eq!(result[1].label, "w");
}

#[test]
fn option_filter_nested_all_none() {
    let data: Vec<OuterRecord> = vec![
        OuterRecord { label: "a".into(), inner: None },
        OuterRecord { label: "b".into(), inner: None },
        OuterRecord { label: "c".into(), inner: None },
    ];
    let bytes = OuterRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"inner.id": 1});
    let result = OuterRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn option_filter_nested_all_some() {
    let data: Vec<OuterRecord> = vec![
        OuterRecord { label: "a".into(), inner: Some(InnerRecord { id: 1, name: "a".into() }) },
        OuterRecord { label: "b".into(), inner: Some(InnerRecord { id: 2, name: "b".into() }) },
        OuterRecord { label: "c".into(), inner: Some(InnerRecord { id: 3, name: "c".into() }) },
        OuterRecord { label: "d".into(), inner: Some(InnerRecord { id: 4, name: "d".into() }) },
    ];
    let bytes = OuterRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"inner.id": 3});
    let result = OuterRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].label, "c");
}

#[test]
fn option_filter_nested_no_match_value() {
    let data: Vec<OuterRecord> = vec![
        OuterRecord { label: "a".into(), inner: Some(InnerRecord { id: 1, name: "a".into() }) },
        OuterRecord { label: "b".into(), inner: None },
        OuterRecord { label: "c".into(), inner: Some(InnerRecord { id: 2, name: "c".into() }) },
    ];
    let bytes = OuterRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"inner.id": 999});
    let result = OuterRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn option_filter_nested_combined_with_top_level() {
    let data: Vec<OuterRecord> = vec![
        OuterRecord { label: "x".into(), inner: Some(InnerRecord { id: 1, name: "a".into() }) },
        OuterRecord { label: "y".into(), inner: None },
        OuterRecord { label: "x".into(), inner: Some(InnerRecord { id: 2, name: "b".into() }) },
        OuterRecord { label: "x".into(), inner: Some(InnerRecord { id: 3, name: "c".into() }) },
        OuterRecord { label: "z".into(), inner: None },
    ];
    let bytes = OuterRecord::serialize(data.clone()).unwrap();
    let query = serde_json::json!({"label": "x", "inner.id": 2});
    let result = OuterRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].inner.as_ref().unwrap().id, 2);
}
