use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct FilterRecord {
    id: i64,
    value: f64,
}

#[test]
fn float_round_filter_f64_exact_match() {
    let data = vec![
        FilterRecord { id: 1, value: 100.0 },
        FilterRecord { id: 2, value: 200.5 },
        FilterRecord { id: 3, value: 100.0 },
    ];
    let bytes = FilterRecord::serialize(data).unwrap();
    let result = FilterRecord::filter_bytes(&bytes, serde_json::json!({"value": 100.0}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
}

#[test]
fn float_round_filter_f64_no_match() {
    let data = vec![FilterRecord { id: 1, value: 100.0 }, FilterRecord { id: 2, value: 200.5 }];
    let bytes = FilterRecord::serialize(data).unwrap();
    let result = FilterRecord::filter_bytes(&bytes, serde_json::json!({"value": 999.0}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn float_round_filter_f32_exact_match() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct F32Record {
        id: i64,
        value: f32,
    }
    let data = vec![F32Record { id: 1, value: 1.235 }, F32Record { id: 2, value: 9.877 }];
    let bytes = F32Record::serialize(data).unwrap();
    let result = F32Record::filter_bytes(&bytes, serde_json::json!({"value": 1.235}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 1);
}

#[test]
fn float_round_filter_inclusion_f64() {
    let data = vec![
        FilterRecord { id: 1, value: 10.0 },
        FilterRecord { id: 2, value: 20.0 },
        FilterRecord { id: 3, value: 30.0 },
    ];
    let bytes = FilterRecord::serialize(data).unwrap();
    let result = FilterRecord::filter_bytes(&bytes, serde_json::json!({"value": [10.0, 30.0]}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
}

#[test]
fn float_round_filter_float_range() {
    let data = vec![
        FilterRecord { id: 1, value: 5.0 },
        FilterRecord { id: 2, value: 15.0 },
        FilterRecord { id: 3, value: 25.0 },
    ];
    let bytes = FilterRecord::serialize(data).unwrap();
    let result =
        FilterRecord::filter_bytes(&bytes, serde_json::json!({"value": {"start": 10, "end": 20}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
}

#[test]
fn float_round_filter_option_f64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct OptRecord {
        id: i64,
        value: Option<f64>,
    }
    let data = vec![
        OptRecord { id: 1, value: Some(50.0) },
        OptRecord { id: 2, value: None },
        OptRecord { id: 3, value: Some(50.0) },
        OptRecord { id: 4, value: Some(75.0) },
    ];
    let bytes = OptRecord::serialize(data).unwrap();
    let result = OptRecord::filter_bytes(&bytes, serde_json::json!({"value": 50.0}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[1].id, 3);
}

#[test]
fn float_round_filter_large_value() {
    let data = vec![
        FilterRecord { id: 1, value: 500.0 },
        FilterRecord { id: 2, value: 500.01 },
        FilterRecord { id: 3, value: 499.99 },
    ];
    let bytes = FilterRecord::serialize(data).unwrap();
    let result = FilterRecord::filter_bytes(&bytes, serde_json::json!({"value": 500.0}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 1);
}
