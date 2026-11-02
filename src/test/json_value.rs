use crate as pco_pack;
use crate::{PcoPack, PcoSerde};
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Default, PcoPack)]
struct JsonRecord {
    val: serde_json::Value,
}

#[test]
fn json_filter_null_query_error_includes_field() {
    let data = vec![JsonRecord { val: json!({"a": 1}) }];
    let bytes = JsonRecord::serialize(data).unwrap();

    let result = JsonRecord::filter_bytes(&bytes, json!({"val": null}), &[]);
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to resolve filter for field 'val'"), "error message: {err}");
    assert!(err.contains("Cannot filter JSON Value column with null query value"), "error message: {err}");
}

#[test]
fn json_value_write_borrowed_matches_owned_bytes() {
    let mut values: Vec<serde_json::Value> = vec![
        json!({"a": [1.234, 2.0, -3, "x", null, true], "b": {"c": 5.6789, "d": [0.005, -0.0051]}}),
        json!([42, 3.14159, "s", {"k": [false, {"deep": 9.999999}]}]),
        json!(null),
        json!(42),
        // (f * scale) overflows to infinity here; both paths must coerce to 0.
        json!([1e308, -1e308]),
    ];
    for i in 0..300 {
        values.push(json!([i as f64 / 3.0, i]));
    }
    for float_round in [0u32, 1, 2, 5] {
        let owned = values.clone();
        let refs: Vec<&serde_json::Value> = values.iter().collect();
        let a = <serde_json::Value as PcoSerde>::write(owned, float_round, Default::default()).unwrap();
        let b = <serde_json::Value as PcoSerde>::write_borrowed(&refs, float_round, Default::default()).unwrap();
        assert_eq!(a, b, "owned and borrowed bytes differ for float_round={float_round}");
    }
}

#[test]
fn json_value_roundtrip() {
    let data = vec![
        JsonRecord { val: json!({"name": "alice", "age": 30, "active": true}) },
        JsonRecord { val: json!({"name": "bob", "age": 25, "active": false}) },
        JsonRecord { val: json!([1, 2, 3, 4]) },
        JsonRecord { val: json!("hello world") },
        JsonRecord { val: json!(null) },
        JsonRecord { val: json!(42.5) },
        JsonRecord { val: json!(true) },
    ];

    let bytes = JsonRecord::serialize(data.clone()).unwrap();
    let result = JsonRecord::deserialize(&bytes).unwrap();
    assert_eq!(result, data);
}

#[test]
fn json_value_empty_column() {
    let bytes = JsonRecord::serialize(Vec::<JsonRecord>::new()).unwrap();
    let result = JsonRecord::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn json_value_nested_structures() {
    let data = vec![
        JsonRecord { val: json!({"user": {"id": 1, "tags": ["admin", "verified"], "metadata": null}}) },
        JsonRecord { val: json!({"user": {"id": 2, "tags": ["user"], "metadata": {"created": "2024-01-01"}}}) },
    ];

    let bytes = JsonRecord::serialize(data.clone()).unwrap();
    let result = JsonRecord::deserialize(&bytes).unwrap();
    assert_eq!(result, data);
}

#[test]
fn json_value_filter_exact_i64() {
    let data = vec![
        JsonRecord { val: json!(10) },
        JsonRecord { val: json!(20) },
        JsonRecord { val: json!(30) },
        JsonRecord { val: json!(20) },
        JsonRecord { val: json!(40) },
    ];
    let bytes = JsonRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": 20});
    let result = JsonRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].val, json!(20));
    assert_eq!(result[1].val, json!(20));
}

#[test]
fn json_value_filter_exact_string() {
    let data = vec![
        JsonRecord { val: json!("apple") },
        JsonRecord { val: json!("banana") },
        JsonRecord { val: json!("apple") },
        JsonRecord { val: json!("cherry") },
    ];
    let bytes = JsonRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": "apple"});
    let result = JsonRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].val, json!("apple"));
    assert_eq!(result[1].val, json!("apple"));
}

#[test]
fn json_value_filter_exact_f64() {
    let data = vec![
        JsonRecord { val: json!(1.5) },
        JsonRecord { val: json!(2.5) },
        JsonRecord { val: json!(3.5) },
        JsonRecord { val: json!(2.5) },
        JsonRecord { val: json!(4.5) },
    ];
    let bytes = JsonRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": 2.5});
    let result = JsonRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].val, json!(2.5));
    assert_eq!(result[1].val, json!(2.5));
}

#[test]
fn json_value_filter_inclusion_not_supported() {
    let data = vec![
        JsonRecord { val: json!(10) },
        JsonRecord { val: json!(20) },
        JsonRecord { val: json!(30) },
        JsonRecord { val: json!(40) },
        JsonRecord { val: json!(50) },
    ];
    let bytes = JsonRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": [20, 40]});
    let result = JsonRecord::filter_bytes(&bytes, query, &[]);
    assert!(result.is_err());
}

#[test]
fn json_value_filter_no_match() {
    let data = vec![JsonRecord { val: json!(1) }, JsonRecord { val: json!(2) }, JsonRecord { val: json!(3) }];
    let bytes = JsonRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": 99});
    let result = JsonRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn json_value_filter_range_not_supported() {
    let data = vec![JsonRecord { val: json!("apple") }, JsonRecord { val: json!("banana") }];
    let bytes = JsonRecord::serialize(data.clone()).unwrap();

    let query = serde_json::json!({"val": {"start": 1, "end": 10}});
    let result = JsonRecord::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}
