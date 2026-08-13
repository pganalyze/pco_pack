use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(index = [id])]
struct SensorRecord {
    id: i64,
    temperature: f64,
    label: String,
}

#[test]
fn test_index_filter() {
    let data = vec![
        SensorRecord { id: 1, temperature: 20.0, label: "a".into() },
        SensorRecord { id: 2, temperature: 25.0, label: "b".into() },
        SensorRecord { id: 1, temperature: 21.0, label: "c".into() },
        SensorRecord { id: 3, temperature: 30.0, label: "d".into() },
        SensorRecord { id: 2, temperature: 26.0, label: "e".into() },
    ];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();
    assert_eq!(5, SensorRecord::deserialize(&bytes).unwrap().len());

    let results = SensorRecord::filter_bytes(&bytes, serde_json::json!({"id": 2}), &[]).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, 2);
    assert_eq!(results[1].id, 2);
}

#[test]
fn test_non_index_filter() {
    let data = vec![
        SensorRecord { id: 1, temperature: 20.0, label: "a".into() },
        SensorRecord { id: 2, temperature: 25.0, label: "b".into() },
        SensorRecord { id: 1, temperature: 21.0, label: "c".into() },
        SensorRecord { id: 3, temperature: 30.0, label: "d".into() },
        SensorRecord { id: 2, temperature: 26.0, label: "e".into() },
    ];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();
    let results = SensorRecord::filter_bytes(&bytes, serde_json::json!({"temperature": 25.0}), &[]).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 2);
    assert_eq!(results[0].temperature, 25.0);
}

#[test]
fn test_non_index_string_filter() {
    let data = vec![
        SensorRecord { id: 1, temperature: 20.0, label: "a".into() },
        SensorRecord { id: 2, temperature: 25.0, label: "b".into() },
        SensorRecord { id: 1, temperature: 21.0, label: "c".into() },
        SensorRecord { id: 3, temperature: 30.0, label: "d".into() },
        SensorRecord { id: 2, temperature: 26.0, label: "e".into() },
    ];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();

    let results = SensorRecord::filter_bytes(&bytes, serde_json::json!({"label": "c"}), &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 1);
    assert_eq!(results[0].label, "c");
}

#[test]
fn test_multi_column_filter() {
    let data = vec![
        SensorRecord { id: 1, temperature: 20.0, label: "a".into() },
        SensorRecord { id: 2, temperature: 25.0, label: "b".into() },
        SensorRecord { id: 1, temperature: 21.0, label: "c".into() },
        SensorRecord { id: 3, temperature: 30.0, label: "d".into() },
        SensorRecord { id: 2, temperature: 26.0, label: "e".into() },
    ];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();

    let results = SensorRecord::filter_bytes(&bytes, serde_json::json!({"id": 1, "temperature": 21.0}), &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 1);
    assert_eq!(results[0].temperature, 21.0);
}

#[test]
fn test_filter_cache_clearing() {
    let data = vec![
        SensorRecord { id: 1, temperature: 20.0, label: "a".into() },
        SensorRecord { id: 2, temperature: 25.0, label: "b".into() },
        SensorRecord { id: 1, temperature: 21.0, label: "c".into() },
        SensorRecord { id: 3, temperature: 30.0, label: "d".into() },
        SensorRecord { id: 2, temperature: 26.0, label: "e".into() },
    ];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();

    let results = SensorRecord::filter_bytes(&bytes, serde_json::json!({"id": 1}), &[]).unwrap();
    assert_eq!(results.len(), 2);

    let results = SensorRecord::filter_bytes(&bytes, serde_json::json!({"temperature": 25.0}), &[]).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_project_subset_fields() {
    let data = vec![
        SensorRecord { id: 1, temperature: 20.0, label: "a".into() },
        SensorRecord { id: 2, temperature: 25.0, label: "b".into() },
        SensorRecord { id: 3, temperature: 30.0, label: "c".into() },
    ];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();

    let mut results = SensorRecord::filter_bytes(&bytes, serde_json::json!({}), &["id", "temperature"]).unwrap();
    results.sort_by_key(|r| r.id);

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].id, 1);
    assert_eq!(results[0].temperature, 20.0);
    assert_eq!(results[0].label, "");

    assert_eq!(results[1].id, 2);
    assert_eq!(results[1].temperature, 25.0);
    assert_eq!(results[1].label, "");

    assert_eq!(results[2].id, 3);
    assert_eq!(results[2].temperature, 30.0);
    assert_eq!(results[2].label, "");
}

#[test]
fn test_project_single_field() {
    let data = vec![
        SensorRecord { id: 10, temperature: 100.0, label: "x".into() },
        SensorRecord { id: 20, temperature: 200.0, label: "y".into() },
    ];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();

    let mut results = SensorRecord::filter_bytes(&bytes, serde_json::json!({}), &["label"]).unwrap();
    results.sort_by_key(|r| r.id);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, 10);
    assert_eq!(results[0].temperature, 0.0);
    assert_eq!(results[0].label, "x");

    assert_eq!(results[1].id, 20);
    assert_eq!(results[1].temperature, 0.0);
    assert_eq!(results[1].label, "y");
}

#[test]
fn test_project_empty_fields_keeps_all() {
    let data = vec![SensorRecord { id: 1, temperature: 20.0, label: "a".into() }];

    let bytes = SensorRecord::serialize(data.clone()).unwrap();

    let mut results = SensorRecord::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    results.sort_by_key(|r| r.id);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 1);
    assert_eq!(results[0].temperature, 20.0);
    assert_eq!(results[0].label, "a");
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(timestamp = ts, index = [device_id])]
struct TelemetryRecord {
    device_id: i64,
    ts: i64,
    temperature: f64,
    label: String,
}

#[test]
fn test_resolve_fields_empty_returns_all() {
    let fields = SensorRecord::resolve_fields(&serde_json::json!({}), &[]).unwrap();
    assert_eq!(fields, ["id", "temperature", "label"]);
}

#[test]
fn test_resolve_fields_index_not_included() {
    let fields = SensorRecord::resolve_fields(&serde_json::json!({}), &["temperature"]).unwrap();
    assert_eq!(fields, ["temperature"]);
}

#[test]
fn test_resolve_fields_index_not_included_with_query() {
    let fields = SensorRecord::resolve_fields(&serde_json::json!({"temperature": 25.0}), &["label"]).unwrap();
    assert_eq!(fields, ["temperature", "label"]);
}

#[test]
fn test_resolve_fields_timestamp_not_included() {
    let fields = TelemetryRecord::resolve_fields(&serde_json::json!({}), &["temperature"]).unwrap();
    assert_eq!(fields, ["temperature"]);
}

#[test]
fn test_resolve_fields_unknown_field_errors() {
    let err = SensorRecord::resolve_fields(&serde_json::json!({}), &["unknown"]).unwrap_err();
    assert!(err.to_string().contains("Unknown field: unknown"));
}

#[test]
fn test_resolve_fields_nested_path_errors() {
    let err = SensorRecord::resolve_fields(&serde_json::json!({}), &["some.nested"]).unwrap_err();
    assert!(err.to_string().contains("Nested field path"));
}
