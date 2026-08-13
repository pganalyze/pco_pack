use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct BoolRow {
    active: bool,
}

#[test]
fn bool_range_filter_errors() {
    let data = vec![BoolRow { active: true }, BoolRow { active: false }];
    let bytes = BoolRow::serialize(data).unwrap();

    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"active": {"start": true, "end": false}}), &[]);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("boolean") || err_msg.contains("numeric"),
        "error message should mention expected type: {}",
        err_msg
    );
}

#[test]
fn bool_range_filter_mixed_types_errors() {
    let data = vec![BoolRow { active: true }];
    let bytes = BoolRow::serialize(data).unwrap();

    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"active": {"start": 0, "end": true}}), &[]);
    assert!(result.is_err());
}

#[test]
fn bool_exact_match_still_works() {
    let data = vec![BoolRow { active: true }, BoolRow { active: false }, BoolRow { active: true }];
    let bytes = BoolRow::serialize(data.clone()).unwrap();

    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"active": true}), &[]).unwrap();
    assert_eq!(result.len(), 2);

    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"active": false}), &[]).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn bool_inclusion_filter_supported() {
    let data = vec![BoolRow { active: true }, BoolRow { active: false }, BoolRow { active: true }];
    let bytes = BoolRow::serialize(data.clone()).unwrap();

    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"active": [true, false]}), &[]).unwrap();
    assert_eq!(result.len(), 3);

    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"active": [true]}), &[]).unwrap();
    assert_eq!(result.len(), 2);
}
