use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct U8 {
    field: u8,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct U16 {
    field: u16,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct U32 {
    field: u32,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct U64 {
    field: u64,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct I8 {
    field: i8,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct I16 {
    field: i16,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct I32 {
    field: i32,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct I64 {
    field: i64,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct F16 {
    field: half::f16,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct F32 {
    field: f32,
}
#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct F64 {
    field: f64,
}

macro_rules! numeric_roundtrip {
    ($name:ident, $wrap:ty, $data:expr) => {
        #[test]
        fn $name() {
            let data: Vec<$wrap> = $data;
            let bytes = <$wrap as PcoPack>::serialize(data.clone()).unwrap();
            let result = <$wrap as PcoPack>::deserialize(&bytes).unwrap();
            assert_eq!(data, result);
        }
    };
}

numeric_roundtrip!(
    u8_roundtrip,
    U8,
    vec![
        U8 { field: 0 },
        U8 { field: 1 },
        U8 { field: 254 },
        U8 { field: 255 },
        U8 { field: 42 },
        U8 { field: 0 },
        U8 { field: 127 }
    ]
);
numeric_roundtrip!(
    u16_roundtrip,
    U16,
    vec![U16 { field: 0 }, U16 { field: 1 }, U16 { field: 1000 }, U16 { field: 32767 }, U16 { field: 65535 }]
);
numeric_roundtrip!(
    u32_roundtrip,
    U32,
    vec![
        U32 { field: 0 },
        U32 { field: 1 },
        U32 { field: 1000 },
        U32 { field: 32767 },
        U32 { field: 65535 },
        U32 { field: 100000 },
        U32 { field: u32::MAX }
    ]
);
numeric_roundtrip!(
    u64_roundtrip,
    U64,
    vec![
        U64 { field: 0 },
        U64 { field: 1 },
        U64 { field: 1000 },
        U64 { field: 32767 },
        U64 { field: 65535 },
        U64 { field: 100000 },
        U64 { field: u64::MAX }
    ]
);
numeric_roundtrip!(
    i8_roundtrip,
    I8,
    vec![I8 { field: -128 }, I8 { field: -1 }, I8 { field: 0 }, I8 { field: 1 }, I8 { field: 127 }]
);
numeric_roundtrip!(
    i16_roundtrip,
    I16,
    vec![I16 { field: -32768 }, I16 { field: -1 }, I16 { field: 0 }, I16 { field: 1 }, I16 { field: 32767 }]
);
numeric_roundtrip!(
    i32_roundtrip,
    I32,
    vec![I32 { field: -1000000 }, I32 { field: -1 }, I32 { field: 0 }, I32 { field: 1 }, I32 { field: 1000000 }]
);
numeric_roundtrip!(
    i64_roundtrip,
    I64,
    vec![I64 { field: -1000000000 }, I64 { field: -1 }, I64 { field: 0 }, I64 { field: 1 }, I64 { field: 1000000000 }]
);
numeric_roundtrip!(
    f16_roundtrip,
    F16,
    vec![
        F16 { field: half::f16::from_f64(0.0) },
        F16 { field: half::f16::from_f64(1.0) },
        F16 { field: half::f16::from_f64(-1.0) },
        F16 { field: half::f16::from_f64(3.14) },
        F16 { field: half::f16::from_f64(0.5) },
        F16 { field: half::f16::from_f64(100.0) },
        F16 { field: half::f16::from_f64(-100.0) },
        F16 { field: half::f16::from_f64(0.001) },
        F16 { field: half::f16::from_f64(200.0) }
    ]
);

fn coerce_roundtrip<S: PcoPack, T: PcoPack>(data: Vec<S>, expected: Vec<T>)
where
    S: std::fmt::Debug + PartialEq,
    T: std::fmt::Debug + PartialEq,
{
    let buf = S::serialize(data).unwrap();
    let result = T::deserialize(&buf).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn coerce_i32_to_i64() {
    let data: Vec<I32> = vec![I32 { field: 42 }, I32 { field: -100 }, I32 { field: 0 }, I32 { field: 1000000 }];
    let expected: Vec<I64> = data.iter().map(|v| I64 { field: v.field as i64 }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_u32_to_u64() {
    let data: Vec<U32> =
        vec![U32 { field: 0 }, U32 { field: 1 }, U32 { field: 65535 }, U32 { field: 100000 }, U32 { field: u32::MAX }];
    let expected: Vec<U64> = data.iter().map(|v| U64 { field: v.field as u64 }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_i64_to_f64() {
    let data: Vec<I64> = vec![
        I64 { field: -1000000000 },
        I64 { field: -1 },
        I64 { field: 0 },
        I64 { field: 1 },
        I64 { field: 1000000000 },
    ];
    let expected: Vec<F64> = data.iter().map(|v| F64 { field: v.field as f64 }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_f32_to_f64() {
    let data: Vec<F32> =
        vec![F32 { field: 0.0 }, F32 { field: 1.0 }, F32 { field: -1.0 }, F32 { field: 3.14159 }, F32 { field: 1e10 }];
    let expected: Vec<F64> = data.iter().map(|v| F64 { field: v.field as f64 }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_u8_to_i32() {
    let data: Vec<U8> = vec![U8 { field: 0 }, U8 { field: 1 }, U8 { field: 127 }, U8 { field: 254 }, U8 { field: 255 }];
    let expected: Vec<I32> = data.iter().map(|v| I32 { field: v.field as i32 }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_i8_to_u16() {
    let data: Vec<I8> = vec![I8 { field: -128 }, I8 { field: -1 }, I8 { field: 0 }, I8 { field: 1 }, I8 { field: 127 }];
    let expected: Vec<U16> =
        vec![U16 { field: 0 }, U16 { field: 0 }, U16 { field: 0 }, U16 { field: 1 }, U16 { field: 127 }];
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_u64_to_f64() {
    let data: Vec<U64> =
        vec![U64 { field: 0 }, U64 { field: 1 }, U64 { field: 65535 }, U64 { field: 100000 }, U64 { field: 1000000 }];
    let expected: Vec<F64> = data.iter().map(|v| F64 { field: v.field as f64 }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_same_type() {
    let data: Vec<I32> = vec![I32 { field: 42 }, I32 { field: -100 }, I32 { field: 0 }];
    let expected: Vec<I32> = data.clone();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_i32_to_u8_overflow() {
    let data: Vec<I32> = vec![I32 { field: 42 }, I32 { field: 300 }, I32 { field: 0 }];
    let buf = I32::serialize(data).unwrap();
    let result = U8::deserialize(&buf).unwrap();
    assert_eq!(result, vec![U8 { field: 42 }, U8 { field: 255 }, U8 { field: 0 }]);
}

#[test]
fn coerce_i64_to_i8() {
    let data: Vec<I64> =
        vec![I64 { field: -100 }, I64 { field: 0 }, I64 { field: 100 }, I64 { field: 200 }, I64 { field: -200 }];
    let expected: Vec<I8> =
        vec![I8 { field: -100 }, I8 { field: 0 }, I8 { field: 100 }, I8 { field: 127 }, I8 { field: -128 }];
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_i64_to_u8() {
    let data: Vec<I64> =
        vec![I64 { field: -10 }, I64 { field: 0 }, I64 { field: 100 }, I64 { field: 300 }, I64 { field: 1000 }];
    let expected: Vec<U8> =
        vec![U8 { field: 0 }, U8 { field: 0 }, U8 { field: 100 }, U8 { field: 255 }, U8 { field: 255 }];
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_u64_to_u8() {
    let data: Vec<U64> =
        vec![U64 { field: 0 }, U64 { field: 100 }, U64 { field: 200 }, U64 { field: 300 }, U64 { field: 1000 }];
    let expected: Vec<U8> =
        vec![U8 { field: 0 }, U8 { field: 100 }, U8 { field: 200 }, U8 { field: 255 }, U8 { field: 255 }];
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_u64_to_i8() {
    let data: Vec<U64> =
        vec![U64 { field: 0 }, U64 { field: 50 }, U64 { field: 100 }, U64 { field: 200 }, U64 { field: 500 }];
    let expected: Vec<I8> =
        vec![I8 { field: 0 }, I8 { field: 50 }, I8 { field: 100 }, I8 { field: 127 }, I8 { field: 127 }];
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_i32_to_i16() {
    let data: Vec<I32> = vec![
        I32 { field: -30000 },
        I32 { field: 0 },
        I32 { field: 30000 },
        I32 { field: 40000 },
        I32 { field: -40000 },
    ];
    let expected: Vec<I16> = vec![
        I16 { field: -30000 },
        I16 { field: 0 },
        I16 { field: 30000 },
        I16 { field: 32767 },
        I16 { field: -32768 },
    ];
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_f64_to_f32() {
    let data: Vec<F64> = vec![
        F64 { field: 0.0 },
        F64 { field: 1.0 },
        F64 { field: -1.0 },
        F64 { field: 3.14159 },
        F64 { field: 1e30 },
        F64 { field: -1e30 },
    ];
    let expected: Vec<F32> = data.iter().map(|v| F32 { field: v.field as f32 }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_f16_to_f32() {
    let data: Vec<F16> = vec![
        F16 { field: half::f16::from_f64(0.0) },
        F16 { field: half::f16::from_f64(1.0) },
        F16 { field: half::f16::from_f64(-1.0) },
        F16 { field: half::f16::from_f64(3.14) },
        F16 { field: half::f16::from_f64(100.0) },
    ];
    let expected: Vec<F32> = data.iter().map(|v| F32 { field: v.field.to_f32() }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_f16_to_f64() {
    let data: Vec<F16> = vec![
        F16 { field: half::f16::from_f64(0.0) },
        F16 { field: half::f16::from_f64(1.0) },
        F16 { field: half::f16::from_f64(-1.0) },
        F16 { field: half::f16::from_f64(3.14) },
        F16 { field: half::f16::from_f64(100.0) },
    ];
    let expected: Vec<F64> = data.iter().map(|v| F64 { field: v.field.to_f64() }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_f32_to_f16() {
    let data: Vec<F32> =
        vec![F32 { field: 0.0 }, F32 { field: 1.0 }, F32 { field: -1.0 }, F32 { field: 3.14 }, F32 { field: 100.0 }];
    let expected: Vec<F16> = data.iter().map(|v| F16 { field: half::f16::from_f32(v.field) }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_f64_to_f16() {
    let data: Vec<F64> =
        vec![F64 { field: 0.0 }, F64 { field: 1.0 }, F64 { field: -1.0 }, F64 { field: 3.14 }, F64 { field: 100.0 }];
    let expected: Vec<F16> = data.iter().map(|v| F16 { field: half::f16::from_f64(v.field) }).collect();
    coerce_roundtrip(data, expected);
}

#[test]
fn f16_filter_exact() {
    let data: Vec<F16> = vec![
        F16 { field: half::f16::from_f64(1.0) },
        F16 { field: half::f16::from_f64(2.0) },
        F16 { field: half::f16::from_f64(3.0) },
        F16 { field: half::f16::from_f64(1.0) },
    ];
    let bytes = F16::serialize(data).unwrap();
    let results = F16::filter_bytes(&bytes, serde_json::json!({"field": 1.0}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].field, half::f16::from_f64(1.0));
    assert_eq!(results[1].field, half::f16::from_f64(1.0));
}

#[test]
fn f16_filter_range() {
    let data: Vec<F16> = vec![
        F16 { field: half::f16::from_f64(1.0) },
        F16 { field: half::f16::from_f64(2.0) },
        F16 { field: half::f16::from_f64(3.0) },
        F16 { field: half::f16::from_f64(4.0) },
    ];
    let bytes = F16::serialize(data).unwrap();
    let results = F16::filter_bytes(&bytes, serde_json::json!({"field": {"start": 2, "end": 3}}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].field, half::f16::from_f64(2.0));
    assert_eq!(results[1].field, half::f16::from_f64(3.0));
}

#[test]
fn f16_filter_inclusion() {
    let data: Vec<F16> = vec![
        F16 { field: half::f16::from_f64(1.0) },
        F16 { field: half::f16::from_f64(2.0) },
        F16 { field: half::f16::from_f64(3.0) },
        F16 { field: half::f16::from_f64(4.0) },
    ];
    let bytes = F16::serialize(data).unwrap();
    let results = F16::filter_bytes(&bytes, serde_json::json!({"field": [1.0, 3.0]}), &[]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].field, half::f16::from_f64(1.0));
    assert_eq!(results[1].field, half::f16::from_f64(3.0));
}

#[test]
fn coerce_f64_to_i32() {
    let data: Vec<F64> =
        vec![F64 { field: 0.0 }, F64 { field: 1.5 }, F64 { field: -1.5 }, F64 { field: 3.14159 }, F64 { field: 100.0 }];
    let expected: Vec<I32> =
        vec![I32 { field: 0 }, I32 { field: 1 }, I32 { field: -1 }, I32 { field: 3 }, I32 { field: 100 }];
    coerce_roundtrip(data, expected);
}

#[test]
fn coerce_f64_to_i32_overflow() {
    let data: Vec<F64> = vec![
        F64 { field: f64::from(i32::MAX) + 1000.0 },
        F64 { field: f64::from(i32::MIN) - 1000.0 },
        F64 { field: 42.0 },
    ];
    let expected: Vec<I32> = vec![I32 { field: i32::MAX }, I32 { field: i32::MIN }, I32 { field: 42 }];
    coerce_roundtrip(data, expected);
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct F32Record {
    id: i64,
    value: f32,
}

#[test]
fn f32_float_range_basic() {
    let data =
        vec![F32Record { id: 1, value: 5.0 }, F32Record { id: 2, value: 15.0 }, F32Record { id: 3, value: 25.0 }];
    let bytes = F32Record::serialize(data).unwrap();
    let result =
        F32Record::filter_bytes(&bytes, serde_json::json!({"value": {"start": 10.0, "end": 20.0}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
}

#[test]
fn f32_float_range_edge_precision() {
    let data = vec![
        F32Record { id: 1, value: 999999.5 },
        F32Record { id: 2, value: 1000000.5 },
        F32Record { id: 3, value: 1000001.5 },
    ];
    let bytes = F32Record::serialize(data).unwrap();

    let result =
        F32Record::filter_bytes(&bytes, serde_json::json!({"value": {"start": 999999.0, "end": 1000001.0}}), &[])
            .unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn f32_float_range_tight_boundary() {
    let data = vec![F32Record { id: 1, value: 0.1 }, F32Record { id: 2, value: 0.2 }, F32Record { id: 3, value: 0.3 }];
    let bytes = F32Record::serialize(data).unwrap();

    let result =
        F32Record::filter_bytes(&bytes, serde_json::json!({"value": {"start": 0.15, "end": 0.25}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
}

#[test]
fn f32_float_range_no_match() {
    let data = vec![F32Record { id: 1, value: 5.0 }, F32Record { id: 2, value: 10.0 }];
    let bytes = F32Record::serialize(data).unwrap();

    let result =
        F32Record::filter_bytes(&bytes, serde_json::json!({"value": {"start": 20.0, "end": 30.0}}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn f32_float_range_with_float_round() {
    #[derive(Debug, Clone, PartialEq, Default, PcoPack)]
    #[pco_pack(float_round = 1)]
    struct RoundedF32Record {
        id: i64,
        value: f32,
    }

    let data = vec![
        RoundedF32Record { id: 1, value: 5.0 },
        RoundedF32Record { id: 2, value: 15.0 },
        RoundedF32Record { id: 3, value: 25.0 },
    ];
    let bytes = RoundedF32Record::serialize(data).unwrap();

    let result =
        RoundedF32Record::filter_bytes(&bytes, serde_json::json!({"value": {"start": 10.0, "end": 20.0}}), &[])
            .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
}

#[test]
fn f32_float_range_negative_values() {
    let data =
        vec![F32Record { id: 1, value: -15.0 }, F32Record { id: 2, value: -5.0 }, F32Record { id: 3, value: 5.0 }];
    let bytes = F32Record::serialize(data).unwrap();

    let result =
        F32Record::filter_bytes(&bytes, serde_json::json!({"value": {"start": -10.0, "end": 0.0}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
}
