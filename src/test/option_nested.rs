use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct DoubleOptRow {
    value: Option<Option<i32>>,
}

#[test]
fn option_option_i32_roundtrip() {
    let data = vec![
        DoubleOptRow { value: Some(Some(1)) },
        DoubleOptRow { value: Some(None) },
        DoubleOptRow { value: None },
        DoubleOptRow { value: Some(Some(-42)) },
        DoubleOptRow { value: None },
    ];
    let bytes = DoubleOptRow::serialize(data.clone()).unwrap();
    let result = DoubleOptRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn option_option_all_some_none() {
    let data = vec![
        DoubleOptRow { value: Some(None) },
        DoubleOptRow { value: Some(None) },
        DoubleOptRow { value: Some(None) },
    ];
    let bytes = DoubleOptRow::serialize(data.clone()).unwrap();
    let result = DoubleOptRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn option_option_all_none() {
    let data = vec![DoubleOptRow { value: None }, DoubleOptRow { value: None }];
    let bytes = DoubleOptRow::serialize(data.clone()).unwrap();
    let result = DoubleOptRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn option_option_all_some_some() {
    let data = vec![
        DoubleOptRow { value: Some(Some(10)) },
        DoubleOptRow { value: Some(Some(20)) },
        DoubleOptRow { value: Some(Some(30)) },
    ];
    let bytes = DoubleOptRow::serialize(data.clone()).unwrap();
    let result = DoubleOptRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn option_option_filter_exact_match() {
    let data = vec![
        DoubleOptRow { value: Some(Some(1)) },
        DoubleOptRow { value: Some(None) },
        DoubleOptRow { value: None },
        DoubleOptRow { value: Some(Some(42)) },
        DoubleOptRow { value: Some(Some(42)) },
    ];
    let bytes = DoubleOptRow::serialize(data.clone()).unwrap();

    let result = DoubleOptRow::filter_bytes(&bytes, serde_json::json!({"value": 42}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value, Some(Some(42)));
    assert_eq!(result[1].value, Some(Some(42)));
}

#[test]
fn option_option_filter_no_match() {
    let data =
        vec![DoubleOptRow { value: Some(Some(1)) }, DoubleOptRow { value: None }, DoubleOptRow { value: Some(None) }];
    let bytes = DoubleOptRow::serialize(data.clone()).unwrap();

    let result = DoubleOptRow::filter_bytes(&bytes, serde_json::json!({"value": 999}), &[]).unwrap();
    assert!(result.is_empty());
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct InnerRecord {
    id: i64,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct DoubleOptStructRow {
    label: String,
    inner: Option<Option<InnerRecord>>,
}

#[test]
fn option_option_struct_filter_nested_field() {
    let data = vec![
        DoubleOptStructRow { label: "a".into(), inner: Some(Some(InnerRecord { id: 1 })) },
        DoubleOptStructRow { label: "b".into(), inner: None },
        DoubleOptStructRow { label: "c".into(), inner: Some(None) },
        DoubleOptStructRow { label: "d".into(), inner: Some(Some(InnerRecord { id: 2 })) },
    ];
    let bytes = DoubleOptStructRow::serialize(data.clone()).unwrap();

    let result = DoubleOptStructRow::filter_bytes(&bytes, serde_json::json!({"inner.id": 2}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].label, "d");
}

#[test]
fn option_option_struct_filter_multiple_matches() {
    let data = vec![
        DoubleOptStructRow { label: "a".into(), inner: Some(Some(InnerRecord { id: 5 })) },
        DoubleOptStructRow { label: "b".into(), inner: None },
        DoubleOptStructRow { label: "c".into(), inner: Some(None) },
        DoubleOptStructRow { label: "d".into(), inner: Some(Some(InnerRecord { id: 5 })) },
        DoubleOptStructRow { label: "e".into(), inner: None },
        DoubleOptStructRow { label: "f".into(), inner: Some(Some(InnerRecord { id: 7 })) },
    ];
    let bytes = DoubleOptStructRow::serialize(data.clone()).unwrap();

    let result = DoubleOptStructRow::filter_bytes(&bytes, serde_json::json!({"inner.id": 5}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].label, "a");
    assert_eq!(result[1].label, "d");
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct NestedVecOptRow {
    values: Vec<Option<Vec<i32>>>,
}

#[test]
fn vec_option_vec_i32_roundtrip() {
    let data = vec![
        NestedVecOptRow { values: vec![Some(vec![1, 2]), None, Some(vec![])] },
        NestedVecOptRow { values: vec![None, None] },
        NestedVecOptRow { values: vec![Some(vec![42])] },
    ];
    let bytes = NestedVecOptRow::serialize(data.clone()).unwrap();
    let result = NestedVecOptRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_option_vec_all_none() {
    let data = vec![NestedVecOptRow { values: vec![None, None, None] }];
    let bytes = NestedVecOptRow::serialize(data.clone()).unwrap();
    let result = NestedVecOptRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_option_vec_mixed() {
    let data = vec![NestedVecOptRow { values: vec![Some(vec![]), Some(vec![1]), None, Some(vec![2, 3])] }];
    let bytes = NestedVecOptRow::serialize(data.clone()).unwrap();
    let result = NestedVecOptRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_option_vec_large() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    #[pco_pack(chunk_size = 5)]
    struct SmallChunkRow {
        values: Vec<Option<Vec<i32>>>,
    }

    let data: Vec<SmallChunkRow> = (0..20)
        .map(|i| {
            let mut vals = Vec::new();
            for j in 0..10 {
                if (i + j) % 3 == 0 {
                    vals.push(Some(vec![i * 10 + j]));
                } else {
                    vals.push(None);
                }
            }
            SmallChunkRow { values: vals }
        })
        .collect();

    let bytes = SmallChunkRow::serialize(data.clone()).unwrap();
    let result = SmallChunkRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}
