use crate as pco_pack;
use crate::PcoPack;
use serde_json::json;

#[derive(PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 2)]
struct JsonRecord {
    id: i32,
    data: serde_json::Value,
}

#[derive(PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 3)]
struct JsonRecordNested {
    id: i32,
    data: serde_json::Value,
}

#[derive(PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 2)]
struct JsonRecordOption {
    id: i32,
    data: Option<serde_json::Value>,
}

#[derive(PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 2)]
struct JsonRecordVec {
    id: i32,
    data: Vec<serde_json::Value>,
}

#[derive(PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 2)]
struct JsonRecordMap {
    id: i32,
    data: std::collections::HashMap<String, serde_json::Value>,
}

#[test]
fn json_value_float_round_roundtrip() {
    let data = vec![
        JsonRecord {
            id: 1,
            data: json!({
                "price": 19.999999999,
                "tax": 1.500000001,
                "count": 42,
                "name": "widget"
            }),
        },
        JsonRecord {
            id: 2,
            data: json!({
                "nested": {
                    "deep": 3.14159265358979
                }
            }),
        },
        JsonRecord { id: 3, data: json!([1.111, 2.222, 3.333, "not a float", null, true]) },
    ];

    let bytes = JsonRecord::serialize(data.clone()).unwrap();
    let result = JsonRecord::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), data.len());
    assert_eq!(result[0].id, 1);
    assert_eq!(result[0].data["price"], json!(20.0));
    assert_eq!(result[0].data["tax"], json!(1.5));
    assert_eq!(result[0].data["count"], json!(42));
    assert_eq!(result[0].data["name"], json!("widget"));

    assert_eq!(result[1].id, 2);
    assert_eq!(result[1].data["nested"]["deep"], json!(3.14));

    assert_eq!(result[2].id, 3);
    assert_eq!(result[2].data[0], json!(1.11));
    assert_eq!(result[2].data[1], json!(2.22));
    assert_eq!(result[2].data[2], json!(3.33));
    assert_eq!(result[2].data[3], json!("not a float"));
    assert_eq!(result[2].data[4], json!(null));
    assert_eq!(result[2].data[5], json!(true));
}

#[test]
fn json_value_option_float_round_roundtrip() {
    let data = vec![
        JsonRecordOption { id: 1, data: Some(json!({"value": 3.14159265358979})) },
        JsonRecordOption { id: 2, data: None },
        JsonRecordOption { id: 3, data: Some(json!({"value": 2.71828182845904})) },
    ];

    let bytes = JsonRecordOption::serialize(data.clone()).unwrap();
    let result = JsonRecordOption::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), data.len());
    assert_eq!(result[0].data.as_ref().unwrap()["value"], json!(3.14));
    assert!(result[1].data.is_none());
    assert_eq!(result[2].data.as_ref().unwrap()["value"], json!(2.72));
}

#[test]
fn json_value_vec_float_round_roundtrip() {
    let data =
        vec![JsonRecordVec { id: 1, data: vec![json!({"v": 0.999999}), json!({"v": 1.000001}), json!({"v": 1.5})] }];

    let bytes = JsonRecordVec::serialize(data.clone()).unwrap();
    let result = JsonRecordVec::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), data.len());
    assert_eq!(result[0].data[0]["v"], json!(1.0));
    assert_eq!(result[0].data[1]["v"], json!(1.0));
    assert_eq!(result[0].data[2]["v"], json!(1.5));
}

#[test]
fn json_value_map_float_round_roundtrip() {
    let mut data_map = std::collections::HashMap::new();
    data_map.insert("a".to_string(), json!({"v": 0.999999}));
    data_map.insert("b".to_string(), json!({"v": 1.000001}));

    let data = vec![JsonRecordMap { id: 1, data: data_map }];

    let bytes = JsonRecordMap::serialize(data.clone()).unwrap();
    let result = JsonRecordMap::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), data.len());
    assert_eq!(result[0].data["a"]["v"], json!(1.0));
    assert_eq!(result[0].data["b"]["v"], json!(1.0));
}

#[test]
fn json_value_different_decimals() {
    let data_3 = vec![
        JsonRecordNested { id: 1, data: json!({"value": 3.14159265358979}) },
        JsonRecordNested { id: 2, data: json!({"value": 2.71828182845904}) },
    ];

    let bytes = JsonRecordNested::serialize(data_3.clone()).unwrap();
    let result = JsonRecordNested::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), data_3.len());
    assert_eq!(result[0].data["value"], json!(3.142));
    assert_eq!(result[1].data["value"], json!(2.718));
}

#[test]
fn json_value_float_round_filter_match() {
    let data = vec![
        JsonRecord { id: 1, data: json!({"value": 3.14}) },
        JsonRecord { id: 2, data: json!({"value": 2.72}) },
        JsonRecord { id: 3, data: json!({"value": 1.41}) },
    ];
    let bytes = JsonRecord::serialize(data).unwrap();
    let filter = serde_json::json!({"id": 1});
    let result = JsonRecord::filter_bytes(&bytes, filter.clone(), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 1);
}
