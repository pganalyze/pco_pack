use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct StrWrapper {
    field: String,
}

#[test]
fn string_roundtrip() {
    let data: Vec<StrWrapper> = vec![
        StrWrapper { field: "hello".into() },
        StrWrapper { field: "world".into() },
        StrWrapper { field: "".into() },
        StrWrapper { field: "unicode: 你好".into() },
        StrWrapper { field: "repeated".into() },
        StrWrapper { field: "repeated".into() },
    ];
    let bytes = StrWrapper::serialize(data.clone()).unwrap();
    let result = StrWrapper::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn empty_string_column() {
    let data: Vec<StrWrapper> = vec![];
    let bytes = StrWrapper::serialize(data).unwrap();
    let result = StrWrapper::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filtered_reader_string() {
    let data: Vec<StrWrapper> = vec![
        StrWrapper { field: "apple".to_string() },
        StrWrapper { field: "banana".to_string() },
        StrWrapper { field: "apple".to_string() },
        StrWrapper { field: "cherry".to_string() },
    ];
    let bytes = StrWrapper::serialize(data.clone()).unwrap();
    let results = StrWrapper::filter_bytes(&bytes, serde_json::json!({"field": "apple"}), &[]).unwrap();
    assert_eq!(results, vec![StrWrapper { field: "apple".to_string() }, StrWrapper { field: "apple".to_string() }]);
}
