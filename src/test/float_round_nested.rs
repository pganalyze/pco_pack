use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(float_round = 2)]
struct InnerFloatRounded {
    id: i64,
    value: f64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct OuterFloatRounded {
    label: String,
    inner: InnerFloatRounded,
}

#[test]
fn float_round_nested_struct_roundtrip() {
    let data = vec![
        OuterFloatRounded { label: "a".to_string(), inner: InnerFloatRounded { id: 1, value: 3.14159 } },
        OuterFloatRounded { label: "b".to_string(), inner: InnerFloatRounded { id: 2, value: 2.71828 } },
        OuterFloatRounded { label: "c".to_string(), inner: InnerFloatRounded { id: 3, value: 1.41421 } },
    ];
    let bytes = OuterFloatRounded::serialize(data.clone()).unwrap();
    let result = OuterFloatRounded::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].inner.value, 3.14);
    assert_eq!(result[1].inner.value, 2.72);
    assert_eq!(result[2].inner.value, 1.41);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(float_round = 2)]
struct InnerF32FloatRounded {
    id: i64,
    value: f32,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct OuterMixedNested {
    label: String,
    inner: InnerF32FloatRounded,
}

#[test]
fn float_round_nested_struct_f32_inner() {
    let data = vec![
        OuterMixedNested { label: "x".to_string(), inner: InnerF32FloatRounded { id: 1, value: 3.14159f32 } },
        OuterMixedNested { label: "y".to_string(), inner: InnerF32FloatRounded { id: 2, value: 2.71828f32 } },
    ];

    let bytes = OuterMixedNested::serialize(data.clone()).unwrap();

    let result = OuterMixedNested::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].inner.value, 3.14);
    assert_eq!(result[1].inner.value, 2.72);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(float_round = 2)]
struct InnerVecFloatRounded {
    id: i64,
    values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 3)]
struct OuterWithVecInner {
    label: String,
    inner: InnerVecFloatRounded,
}

#[test]
fn float_round_nested_struct_vec_inner() {
    let data = vec![
        OuterWithVecInner {
            label: "a".to_string(),
            inner: InnerVecFloatRounded { id: 1, values: vec![1.23456, 9.87654] },
        },
        OuterWithVecInner {
            label: "b".to_string(),
            inner: InnerVecFloatRounded { id: 2, values: vec![3.14159, 2.71828] },
        },
    ];
    let bytes = OuterWithVecInner::serialize(data.clone()).unwrap();
    let result = OuterWithVecInner::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].inner.values[0], 1.23);
    assert_eq!(result[0].inner.values[1], 9.88);
    assert_eq!(result[1].inner.values[0], 3.14);
    assert_eq!(result[1].inner.values[1], 2.72);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct InnerNoFloatRound {
    id: i64,
    value: f64,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 2)]
struct OuterWithFloatRoundOnly {
    label: String,
    outer_value: f64,
    inner: InnerNoFloatRound,
}

#[test]
fn float_round_does_not_inherit_into_nested_struct() {
    let data = vec![OuterWithFloatRoundOnly {
        label: "a".to_string(),
        outer_value: 3.14159265,
        inner: InnerNoFloatRound { id: 1, value: 2.718281828 },
    }];

    let bytes = OuterWithFloatRoundOnly::serialize(data.clone()).unwrap();
    let result = OuterWithFloatRoundOnly::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].outer_value, 3.14);
    assert_eq!(result[0].inner.value, 2.718281828);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct InnerNoFloatRoundF32 {
    value: f32,
}

#[derive(Debug, Clone, PartialEq, PcoPack)]
#[pco_pack(float_round = 1)]
struct OuterWithFloatRoundOnlyF32 {
    outer_value: f32,
    inner: InnerNoFloatRoundF32,
}

#[test]
fn float_round_does_not_inherit_into_nested_struct_f32() {
    let data =
        vec![OuterWithFloatRoundOnlyF32 { outer_value: 9.876f32, inner: InnerNoFloatRoundF32 { value: 1.234567f32 } }];

    let bytes = OuterWithFloatRoundOnlyF32::serialize(data.clone()).unwrap();
    let result = OuterWithFloatRoundOnlyF32::filter_bytes(&bytes, serde_json::json!({}), &[]).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].outer_value, 9.9);
    assert_eq!(result[0].inner.value, 1.234567f32);
}
