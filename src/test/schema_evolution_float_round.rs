use crate as pco_pack;
use crate::PcoPack;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(float_round = 2)]
struct InnerFloatRounded {
    id: i64,
    value: f64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct MetricV1 {
    id: i64,
    value: f64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct MetricV2 {
    id: i64,
    value: f64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct MetricV3 {
    id: i64,
    value: f64,
}

#[test]
fn float_round_cross_version_no_rounding_then_enabled() {
    let data_v1 = vec![
        MetricV1 { id: 1, value: 3.14159 },
        MetricV1 { id: 2, value: 2.71828 },
        MetricV1 { id: 3, value: 1.41421 },
    ];

    let bytes = MetricV1::serialize(data_v1.clone()).unwrap();
    let result = MetricV2::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert!((result[0].value - 3.14159).abs() < 1e-10);
    assert!((result[1].value - 2.71828).abs() < 1e-10);
    assert!((result[2].value - 1.41421).abs() < 1e-10);
}

#[test]
fn float_round_cross_version_different_precision() {
    let data_v2 = vec![
        MetricV2 { id: 1, value: 3.14159 },
        MetricV2 { id: 2, value: 2.71828 },
        MetricV2 { id: 3, value: 1.41421 },
    ];

    let bytes = MetricV2::serialize(data_v2.clone()).unwrap();
    let result = MetricV3::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert!((result[0].value - 3.14).abs() < 1e-10);
    assert!((result[1].value - 2.72).abs() < 1e-10);
    assert!((result[2].value - 1.41).abs() < 1e-10);
}

#[test]
fn float_round_cross_version_enabled_then_disabled() {
    let data_v2 = vec![
        MetricV2 { id: 1, value: 3.14159 },
        MetricV2 { id: 2, value: 2.71828 },
        MetricV2 { id: 3, value: 1.41421 },
    ];

    let bytes = MetricV2::serialize(data_v2.clone()).unwrap();
    let result = MetricV1::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert!((result[0].value - 3.14).abs() < 1e-10);
    assert!((result[1].value - 2.72).abs() < 1e-10);
    assert!((result[2].value - 1.41).abs() < 1e-10);
}

#[test]
fn float_round_cross_version_higher_to_lower_precision() {
    let data_v2 = vec![MetricV2 { id: 1, value: 3.14159 }, MetricV2 { id: 2, value: 2.71828 }];

    let bytes = MetricV2::serialize(data_v2.clone()).unwrap();

    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 1)]
    struct MetricV1Precision {
        id: i64,
        value: f64,
    }

    let result = MetricV1Precision::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!((result[0].value - 3.14).abs() < 1e-10);
    assert!((result[1].value - 2.72).abs() < 1e-10);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
struct OuterNoFloatRound {
    device_id: i64,
    inner: InnerFloatRounded,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct OuterWithFloatRound {
    device_id: i64,
    inner: InnerFloatRounded,
}

#[test]
fn float_round_cross_version_nested_outer_no_float_round() {
    let data = vec![
        OuterNoFloatRound { device_id: 1, inner: InnerFloatRounded { id: 1, value: 3.14159 } },
        OuterNoFloatRound { device_id: 2, inner: InnerFloatRounded { id: 2, value: 2.71828 } },
    ];

    let bytes = OuterNoFloatRound::serialize(data.clone()).unwrap();

    let result = OuterWithFloatRound::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);

    assert!((result[0].inner.value - 3.14).abs() < 1e-10);
    assert!((result[1].inner.value - 2.72).abs() < 1e-10);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct VecMetricV2 {
    id: i64,
    values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct VecMetricV3 {
    id: i64,
    values: Vec<f64>,
}

#[test]
fn float_round_cross_version_vec_different_precision() {
    let data_v2 = vec![
        VecMetricV2 { id: 1, values: vec![1.23456, 9.87654] },
        VecMetricV2 { id: 2, values: vec![3.14159, 2.71828] },
    ];

    let bytes = VecMetricV2::serialize(data_v2.clone()).unwrap();
    let result = VecMetricV3::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);

    assert!((result[0].values[0] - 1.23).abs() < 1e-10);
    assert!((result[0].values[1] - 9.88).abs() < 1e-10);
    assert!((result[1].values[0] - 3.14).abs() < 1e-10);
    assert!((result[1].values[1] - 2.72).abs() < 1e-10);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct MapMetricV2 {
    id: i64,
    metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct MapMetricV3 {
    id: i64,
    metrics: HashMap<String, f64>,
}

#[test]
fn float_round_cross_version_map_different_precision() {
    let data_v2 = vec![MapMetricV2 {
        id: 1,
        metrics: HashMap::from([("temp".to_string(), 3.14159), ("humidity".to_string(), 65.4321)]),
    }];

    let bytes = MapMetricV2::serialize(data_v2.clone()).unwrap();
    let result = MapMetricV3::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 1);

    assert!((result[0].metrics["temp"] - 3.14).abs() < 1e-10);
    assert!((result[0].metrics["humidity"] - 65.43).abs() < 1e-10);
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct OptMetricV2 {
    id: i64,
    value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct OptMetricV3 {
    id: i64,
    value: Option<f64>,
}

#[test]
fn float_round_cross_version_option_different_precision() {
    let data_v2 = vec![
        OptMetricV2 { id: 1, value: Some(3.14159) },
        OptMetricV2 { id: 2, value: None },
        OptMetricV2 { id: 3, value: Some(2.71828) },
    ];

    let bytes = OptMetricV2::serialize(data_v2.clone()).unwrap();
    let result = OptMetricV3::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);

    assert!(result[0].value.is_some());
    assert!((result[0].value.unwrap() - 3.14).abs() < 1e-10);
    assert!(result[1].value.is_none());
    assert!(result[2].value.is_some());
    assert!((result[2].value.unwrap() - 2.72).abs() < 1e-10);
}

#[test]
fn float_round_coerce_f64_to_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct F64R2 {
        id: i64,
        value: f64,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct F32P {
        id: i64,
        value: f32,
    }

    let data = vec![F64R2 { id: 1, value: 3.14159 }, F64R2 { id: 2, value: 2.71828 }, F64R2 { id: 3, value: 1.41421 }];
    let bytes = F64R2::serialize(data).unwrap();
    let result = F32P::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert!((result[0].value - 3.14f32).abs() < 1e-6);
    assert!((result[1].value - 2.72f32).abs() < 1e-6);
    assert!((result[2].value - 1.41f32).abs() < 1e-6);
}

#[test]
fn float_round_coerce_f64_to_f32_both_rounded() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct F64R2B {
        id: i64,
        value: f64,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct F32R3 {
        id: i64,
        value: f32,
    }

    let data = vec![F64R2B { id: 1, value: 3.14159 }, F64R2B { id: 2, value: 2.71828 }];
    let bytes = F64R2B::serialize(data).unwrap();
    let result = F32R3::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!((result[0].value - 3.14f32).abs() < 1e-6);
    assert!((result[1].value - 2.72f32).abs() < 1e-6);
}

#[test]
fn float_round_coerce_f32_to_f64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct F32R3B {
        id: i64,
        value: f32,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct F64P {
        id: i64,
        value: f64,
    }

    let data = vec![F32R3B { id: 1, value: 1.234567 }, F32R3B { id: 2, value: 9.876543 }];
    let bytes = F32R3B::serialize(data).unwrap();
    let result = F64P::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!((result[0].value - 1.235).abs() < 1e-9);
    assert!((result[1].value - 9.877).abs() < 1e-9);
}

#[test]
fn float_round_coerce_f64_to_f16() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct F64R2C {
        id: i64,
        value: f64,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct F16P {
        id: i64,
        value: half::f16,
    }

    let data = vec![F64R2C { id: 1, value: 3.14159 }, F64R2C { id: 2, value: 2.71828 }, F64R2C { id: 3, value: 100.0 }];
    let bytes = F64R2C::serialize(data).unwrap();
    let result = F16P::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, half::f16::from_f64(3.14));
    assert_eq!(result[1].value, half::f16::from_f64(2.72));
    assert_eq!(result[2].value, half::f16::from_f64(100.0));
}

#[test]
fn float_round_coerce_f32_to_f16_both_rounded() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct F32R2 {
        id: i64,
        value: f32,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct F16R3 {
        id: i64,
        value: half::f16,
    }

    let data = vec![F32R2 { id: 1, value: 3.14159 }, F32R2 { id: 2, value: 100.0 }];
    let bytes = F32R2::serialize(data).unwrap();
    let result = F16R3::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value, half::f16::from_f64(3.14));
    assert_eq!(result[1].value, half::f16::from_f64(100.0));
}

#[test]
fn float_round_coerce_f16_to_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct F16R2 {
        id: i64,
        value: half::f16,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct F32PB {
        id: i64,
        value: f32,
    }

    let data = vec![
        F16R2 { id: 1, value: half::f16::from_f64(3.14159) },
        F16R2 { id: 2, value: half::f16::from_f64(2.71828) },
    ];
    let bytes = F16R2::serialize(data).unwrap();
    let result = F32PB::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!((result[0].value - 3.14f32).abs() < 1e-4);
    assert!((result[1].value - 2.72f32).abs() < 1e-4);
}

#[test]
fn float_round_coerce_f16_to_f64() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct F16R2B {
        id: i64,
        value: half::f16,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct F64PB {
        id: i64,
        value: f64,
    }

    let data = vec![
        F16R2B { id: 1, value: half::f16::from_f64(3.14159) },
        F16R2B { id: 2, value: half::f16::from_f64(100.0) },
    ];
    let bytes = F16R2B::serialize(data).unwrap();
    let result = F64PB::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!((result[0].value - 3.14).abs() < 1e-4);
    assert!((result[1].value - 100.0).abs() < 1e-9);
}

/// Verifies float->int truncation works through the FloatRoundStorage layer.
#[test]
fn float_round_coerce_f64_to_i32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct F64R2D {
        id: i64,
        value: f64,
    }
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    struct I32P {
        id: i64,
        value: i32,
    }

    let data = vec![
        F64R2D { id: 1, value: 3.99 },
        F64R2D { id: 2, value: 2.71828 },
        F64R2D { id: 3, value: -1.5 },
        F64R2D { id: 4, value: 100.0 },
    ];
    let bytes = F64R2D::serialize(data).unwrap();
    let result = I32P::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result.len(), 4);
    // 3.99 rounds to 3.99, then truncates to 3
    assert_eq!(result[0].value, 3);
    // 2.71828 rounds to 2.72, then truncates to 2
    assert_eq!(result[1].value, 2);
    // -1.5 rounds to -1.50, then truncates to -1
    assert_eq!(result[2].value, -1);
    assert_eq!(result[3].value, 100);
}
