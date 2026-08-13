use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct FloatRounded {
    id: i64,
    value: f64,
}

#[test]
fn float_round_roundtrip() {
    let data = vec![
        FloatRounded { id: 1, value: 3.14159 },
        FloatRounded { id: 2, value: 2.71828 },
        FloatRounded { id: 3, value: 1.41421 },
    ];
    let bytes = FloatRounded::serialize(data.clone()).unwrap();
    let result = FloatRounded::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, 3.14);
    assert_eq!(result[1].value, 2.72);
    assert_eq!(result[2].value, 1.41);
}

#[test]
fn float_round_precision_loss() {
    let data = vec![FloatRounded { id: 1, value: 1.23456789 }, FloatRounded { id: 2, value: 9.87654321 }];
    let bytes = FloatRounded::serialize(data.clone()).unwrap();
    let result = FloatRounded::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();
    assert_eq!(result[0].value, 1.23);
}

#[test]
fn float_round_zero_value() {
    let data = vec![
        FloatRounded { id: 1, value: 0.0 },
        FloatRounded { id: 2, value: 0.001 },
        FloatRounded { id: 3, value: -0.001 },
    ];
    let bytes = FloatRounded::serialize(data.clone()).unwrap();
    let result = FloatRounded::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result[0].value, 0.0);
    assert_eq!(result[1].value, 0.0);
    assert_eq!(result[2].value, 0.0);
}

#[test]
fn float_round_f32() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 3)]
    struct FloatRoundedF32 {
        id: i64,
        value: f32,
    }

    let data = vec![FloatRoundedF32 { id: 1, value: 1.234567 }, FloatRoundedF32 { id: 2, value: 9.876543 }];
    let bytes = FloatRoundedF32::serialize(data.clone()).unwrap();
    let result = FloatRoundedF32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result[0].value, 1.235);
    assert_eq!(result[1].value, 9.877);
}

#[test]
fn float_round_with_index() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(index = [category], float_round = 2)]
    struct GroupedFloat {
        category: i64,
        value: f64,
    }

    let data = vec![
        GroupedFloat { category: 1, value: 1.234 },
        GroupedFloat { category: 2, value: 5.678 },
        GroupedFloat { category: 1, value: 1.236 },
        GroupedFloat { category: 2, value: 5.672 },
    ];
    let bytes = GroupedFloat::serialize(data.clone()).unwrap();
    let result = GroupedFloat::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 4);
    let cat1: Vec<_> = result.iter().filter(|r| r.category == 1).collect();
    assert_eq!(cat1.len(), 2);
    assert_eq!(cat1[0].value, 1.23);
    assert_eq!(cat1[1].value, 1.24);
    let cat2: Vec<_> = result.iter().filter(|r| r.category == 2).collect();
    assert_eq!(cat2.len(), 2);
    assert_eq!(cat2[0].value, 5.68);
    assert_eq!(cat2[1].value, 5.67);
}

#[test]
fn float_round_f16() {
    #[derive(Debug, Clone, PartialEq, PcoPack)]
    #[pco_pack(float_round = 2)]
    struct FloatRoundedF16 {
        id: i64,
        value: half::f16,
    }

    let data = vec![
        FloatRoundedF16 { id: 1, value: half::f16::from_f64(3.14159) },
        FloatRoundedF16 { id: 2, value: half::f16::from_f64(2.71828) },
        FloatRoundedF16 { id: 3, value: half::f16::from_f64(1.41421) },
    ];
    let bytes = FloatRoundedF16::serialize(data.clone()).unwrap();
    let result = FloatRoundedF16::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].value, half::f16::from_f64(3.14));
    assert_eq!(result[1].value, half::f16::from_f64(2.72));
    assert_eq!(result[2].value, half::f16::from_f64(1.41));
}
