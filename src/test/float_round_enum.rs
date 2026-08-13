use crate as pco_pack;
use crate::PcoPack;

#[derive(Default, PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 2)]
enum FloatEnum {
    #[default]
    None,
    Value(f64),
}

#[derive(Default, PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 3)]
enum FloatEnumMixed {
    #[default]
    None,
    Value(f64),
    Other(i32),
}

#[derive(Default, PcoPack, Debug, PartialEq, Clone)]
#[pco_pack(float_round = 2)]
enum FloatEnumJson {
    #[default]
    None,
    Data(serde_json::Value),
}

#[test]
fn enum_float_round_pco_pack_roundtrip() {
    let data = vec![
        FloatEnum::None,
        FloatEnum::Value(19.999999999),
        FloatEnum::Value(3.14159265358979),
        FloatEnum::Value(0.0),
    ];

    let bytes = FloatEnum::serialize(data.clone()).unwrap();

    let result = FloatEnum::deserialize(&bytes).unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(result[0], FloatEnum::None);
    assert_eq!(result[1], FloatEnum::Value(20.0));
    assert_eq!(result[2], FloatEnum::Value(3.14));
    assert_eq!(result[3], FloatEnum::Value(0.0));
}

#[test]
fn enum_float_round_chunks_no_rounding() {
    let data = vec![FloatEnum::None, FloatEnum::Value(19.999999999), FloatEnum::Value(3.14159265358979)];

    let bytes = FloatEnum::serialize(data.clone()).unwrap();
    let result = FloatEnum::deserialize(&bytes).unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], FloatEnum::None);
    assert_eq!(result[1], FloatEnum::Value(20.0));
    assert_eq!(result[2], FloatEnum::Value(3.14));
}

#[test]
fn enum_float_round_mixed_variants() {
    let data = vec![
        FloatEnumMixed::None,
        FloatEnumMixed::Value(123.456789),
        FloatEnumMixed::Other(42),
        FloatEnumMixed::Value(999.999999),
    ];

    let bytes = FloatEnumMixed::serialize(data.clone()).unwrap();
    let result = FloatEnumMixed::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 4);
    assert_eq!(result[0], FloatEnumMixed::None);
    assert_eq!(result[1], FloatEnumMixed::Value(123.457));
    assert_eq!(result[2], FloatEnumMixed::Other(42));
    assert_eq!(result[3], FloatEnumMixed::Value(1000.0));
}

#[test]
fn enum_float_round_json_value() {
    let data =
        vec![FloatEnumJson::None, FloatEnumJson::Data(serde_json::json!({"price": 19.999999999, "tax": 1.500000001}))];

    let bytes = FloatEnumJson::serialize(data.clone()).unwrap();
    let result = FloatEnumJson::deserialize(&bytes).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0], FloatEnumJson::None);

    match &result[1] {
        FloatEnumJson::Data(v) => {
            assert_eq!(v["price"], serde_json::json!(20.0));
            assert_eq!(v["tax"], serde_json::json!(1.50));
        }
        _ => panic!("Expected Data variant"),
    }
}

#[test]
fn enum_float_round_filter() {
    let data = vec![
        FloatEnum::None,                    // discriminant 0
        FloatEnum::Value(19.999999999),     // discriminant 1
        FloatEnum::Value(3.14159265358979), // discriminant 1
        FloatEnum::Value(0.0),              // discriminant 1
    ];

    let bytes = FloatEnum::serialize(data.clone()).unwrap();
    let filter = serde_json::json!({});
    let result = FloatEnum::filter_bytes(&bytes, filter.clone(), &[]).unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(result[1], FloatEnum::Value(20.0));
    assert_eq!(result[2], FloatEnum::Value(3.14));

    let filter = serde_json::json!({"": {"start": 0, "end": 0}});
    let result = FloatEnum::filter_bytes(&bytes, filter.clone(), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], FloatEnum::None);

    let filter = serde_json::json!({"": {"start": 1, "end": 1}});
    let result = FloatEnum::filter_bytes(&bytes, filter.clone(), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert!(matches!(result[0], FloatEnum::Value(_)));
    assert!(matches!(result[1], FloatEnum::Value(_)));
    assert!(matches!(result[2], FloatEnum::Value(_)));
}
