use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct FloatRow {
    value: f64,
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct F32Row {
    value: f32,
}

#[test]
fn f64_nan_roundtrip() {
    let data = vec![FloatRow { value: f64::NAN }, FloatRow { value: 1.0 }];
    let bytes = FloatRow::serialize(data.clone()).unwrap();
    let result = FloatRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].value.is_nan());
    assert!(!result[1].value.is_nan());
}

#[test]
fn f32_nan_roundtrip() {
    let data = vec![F32Row { value: f32::NAN }, F32Row { value: 1.0 }];
    let bytes = F32Row::serialize(data.clone()).unwrap();
    let result = F32Row::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].value.is_nan());
    assert!(!result[1].value.is_nan());
}

#[test]
fn f64_multiple_nans_roundtrip() {
    let data = vec![FloatRow { value: f64::NAN }, FloatRow { value: f64::NAN }];
    let bytes = FloatRow::serialize(data.clone()).unwrap();
    let result = FloatRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].value.is_nan());
    assert!(result[1].value.is_nan());
}

#[test]
fn f64_infinity_roundtrip() {
    let data = vec![FloatRow { value: f64::INFINITY }, FloatRow { value: 1.0 }];
    let bytes = FloatRow::serialize(data.clone()).unwrap();
    let result = FloatRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].value.is_infinite() && result[0].value > 0.0);
    assert!(!result[1].value.is_infinite());
}

#[test]
fn f64_neg_infinity_roundtrip() {
    let data = vec![FloatRow { value: f64::NEG_INFINITY }, FloatRow { value: 1.0 }];
    let bytes = FloatRow::serialize(data.clone()).unwrap();
    let result = FloatRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].value.is_infinite() && result[0].value < 0.0);
    assert!(!result[1].value.is_infinite());
}

#[test]
fn f32_infinity_roundtrip() {
    let data = vec![F32Row { value: f32::INFINITY }, F32Row { value: 1.0 }];
    let bytes = F32Row::serialize(data.clone()).unwrap();
    let result = F32Row::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].value.is_infinite() && result[0].value > 0.0);
    assert!(!result[1].value.is_infinite());
}

#[test]
fn f32_neg_infinity_roundtrip() {
    let data = vec![F32Row { value: f32::NEG_INFINITY }, F32Row { value: 1.0 }];
    let bytes = F32Row::serialize(data.clone()).unwrap();
    let result = F32Row::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert!(result[0].value.is_infinite() && result[0].value < 0.0);
    assert!(!result[1].value.is_infinite());
}

#[test]
fn f64_mixed_special_values_roundtrip() {
    let data = vec![
        FloatRow { value: f64::NAN },
        FloatRow { value: f64::INFINITY },
        FloatRow { value: f64::NEG_INFINITY },
        FloatRow { value: 0.0 },
        FloatRow { value: -1e308 },
    ];
    let bytes = FloatRow::serialize(data.clone()).unwrap();
    let result = FloatRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 5);
    assert!(result[0].value.is_nan());
    assert!(result[1].value == f64::INFINITY);
    assert!(result[2].value == f64::NEG_INFINITY);
    assert!(!(result[3].value.is_nan() || result[3].value.is_infinite()));
    assert!(!(result[4].value.is_nan() || result[4].value.is_infinite()));
}

#[test]
fn f64_filter_infinity_not_in_finite_range() {
    let data = vec![
        FloatRow { value: 1.0 },
        FloatRow { value: f64::INFINITY },
        FloatRow { value: -5.0 },
        FloatRow { value: f64::INFINITY },
    ];
    let bytes = FloatRow::serialize(data.clone()).unwrap();

    let result =
        FloatRow::filter_bytes(&bytes, serde_json::json!({"value": {"start": 1e308, "end": f64::MAX}}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn f64_filter_neg_infinity_not_in_finite_range() {
    let data = vec![FloatRow { value: f64::NEG_INFINITY }, FloatRow { value: -1e308 }, FloatRow { value: 0.0 }];
    let bytes = FloatRow::serialize(data.clone()).unwrap();

    let result =
        FloatRow::filter_bytes(&bytes, serde_json::json!({"value": {"start": f64::MIN, "end": -1e307}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!(!result[0].value.is_infinite());
}

#[test]
fn f64_filter_nan_no_exact_match() {
    let data = vec![FloatRow { value: f64::NAN }, FloatRow { value: 1.0 }];
    let bytes = FloatRow::serialize(data.clone()).unwrap();

    let result = FloatRow::filter_bytes(&bytes, serde_json::json!({"value": 1.0}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert!(!result[0].value.is_nan());
}

#[test]
fn f64_filter_nan_not_in_range() {
    let data = vec![FloatRow { value: f64::NAN }, FloatRow { value: 50.0 }];
    let bytes = FloatRow::serialize(data.clone()).unwrap();

    let result =
        FloatRow::filter_bytes(&bytes, serde_json::json!({"value": {"start": 0.0, "end": 100.0}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value, 50.0);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct VecFloatRow {
    values: Vec<f64>,
}

#[test]
fn vec_f64_special_values_roundtrip() {
    let data = vec![
        VecFloatRow { values: vec![f64::NAN, 1.0, f64::INFINITY] },
        VecFloatRow { values: vec![] },
        VecFloatRow { values: vec![f64::NEG_INFINITY, -5.0] },
    ];
    let bytes = VecFloatRow::serialize(data.clone()).unwrap();
    let result = VecFloatRow::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 3);
    assert!(result[0].values[0].is_nan());
    assert!(result[0].values[2] == f64::INFINITY);
    assert!(result[2].values[0] == f64::NEG_INFINITY);
}
