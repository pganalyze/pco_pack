use crate as pco_pack;
use crate::{PcoPack, PcoSerde};

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct BoolRow {
    value: bool,
}

#[test]
fn bool_roundtrip() {
    let data: Vec<BoolRow> = vec![
        BoolRow { value: true },
        BoolRow { value: false },
        BoolRow { value: true },
        BoolRow { value: true },
        BoolRow { value: false },
    ];
    let bytes = BoolRow::serialize(data.clone()).unwrap();
    let result = BoolRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

fn serialize_bool_pco_u16(data: Vec<bool>) -> Vec<u8> {
    let u8_data: Vec<u16> = data.into_iter().map(|b| if b { 1 } else { 0 }).collect();
    let config = pco::ChunkConfig::default().with_enable_8_bit(true);
    pco::standalone::simple_compress(&u8_data, &config).unwrap()
}

#[test]
fn bool_backward_compat_roundtrip() {
    let data: Vec<bool> = vec![true, false, true, true, false, false, true, false];
    let pco_buf = serialize_bool_pco_u16(data.clone());
    let mut cursor = std::io::Cursor::new(&pco_buf[..]);
    let mut de_columns = bool::read(&mut cursor, 0, chrono::Duration::zero()).unwrap();
    let row_count = bool::validate_bounds(&mut de_columns).unwrap().unwrap();
    assert_eq!(row_count, data.len());
    let mut row_idx: usize = 0;
    for expected in &data {
        let val = bool::get(&mut de_columns, row_idx).unwrap().unwrap();
        row_idx += 1;
        assert_eq!(val, *expected);
    }
}

#[test]
fn bool_backward_compat_pop_values() {
    let data: Vec<bool> = vec![true, false, true, false, true];
    let pco_buf = serialize_bool_pco_u16(data.clone());
    let mut cursor = std::io::Cursor::new(&pco_buf[..]);
    let mut de_columns = bool::read(&mut cursor, 0, chrono::Duration::zero()).unwrap();
    let mut row_idx: usize = 0;
    for expected in &data {
        let val = bool::get(&mut de_columns, row_idx).unwrap().unwrap();
        row_idx += 1;
        assert_eq!(val, *expected);
    }
}

#[test]
fn filter_bool_true() {
    let data: Vec<BoolRow> = vec![
        BoolRow { value: true },
        BoolRow { value: false },
        BoolRow { value: true },
        BoolRow { value: true },
        BoolRow { value: false },
    ];
    let bytes = BoolRow::serialize(data).unwrap();
    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"value": true}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    assert!(result.iter().all(|r| r.value));
}

#[test]
fn filter_bool_false() {
    let data: Vec<BoolRow> = vec![
        BoolRow { value: true },
        BoolRow { value: false },
        BoolRow { value: true },
        BoolRow { value: true },
        BoolRow { value: false },
    ];
    let bytes = BoolRow::serialize(data).unwrap();
    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"value": false}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|r| !r.value));
}

#[test]
fn filter_bool_only_false() {
    let data: Vec<BoolRow> = vec![BoolRow { value: false }, BoolRow { value: false }, BoolRow { value: false }];
    let bytes = BoolRow::serialize(data).unwrap();
    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"value": false}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"value": true}), &[]).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn filter_bool_only_true() {
    let data: Vec<BoolRow> = vec![BoolRow { value: true }, BoolRow { value: true }, BoolRow { value: true }];
    let bytes = BoolRow::serialize(data).unwrap();
    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"value": true}), &[]).unwrap();
    assert_eq!(result.len(), 3);
    let result = BoolRow::filter_bytes(&bytes, serde_json::json!({"value": false}), &[]).unwrap();
    assert_eq!(result.len(), 0);
}
