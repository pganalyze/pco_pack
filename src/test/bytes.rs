use crate as pco_pack;
use crate::PcoPack;
use serde_bytes::ByteBuf;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct BytesRow {
    value: ByteBuf,
}

fn roundtrip(data: Vec<BytesRow>) {
    let bytes = BytesRow::serialize(data.clone()).unwrap();
    let result = BytesRow::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn test_bytes_empty() {
    roundtrip(Vec::new());
}

#[test]
fn test_bytes_single() {
    roundtrip(vec![BytesRow { value: ByteBuf::from(b"hello".as_slice()) }]);
}

#[test]
fn test_bytes_multiple() {
    let data = vec![
        BytesRow { value: ByteBuf::from(b"binary\x00data".as_slice()) },
        BytesRow { value: ByteBuf::from(b"\xff\xfe\xfd".as_slice()) },
        BytesRow { value: ByteBuf::from(b"".as_slice()) },
        BytesRow { value: ByteBuf::from(b"again".as_slice()) },
    ];
    roundtrip(data);
}

#[test]
fn test_bytes_large() {
    let data: Vec<BytesRow> = (0..1000).map(|i| BytesRow { value: ByteBuf::from(vec![i as u8; 256]) }).collect();
    roundtrip(data);
}

#[test]
fn test_bytes_filter_exact_match() {
    let data = vec![
        BytesRow { value: ByteBuf::from(b"hello".as_slice()) },
        BytesRow { value: ByteBuf::from(b"world".as_slice()) },
        BytesRow { value: ByteBuf::from(b"foo".as_slice()) },
    ];
    let bytes = BytesRow::serialize(data).unwrap();
    let query = serde_json::json!({"value": "68656c6c6f"}); // "hello" in hex
    let result = BytesRow::filter_bytes(&bytes, query, &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].value.as_slice(), b"hello");
}

#[test]
fn test_bytes_filter_no_match() {
    let data = vec![
        BytesRow { value: ByteBuf::from(b"hello".as_slice()) },
        BytesRow { value: ByteBuf::from(b"world".as_slice()) },
    ];
    let bytes = BytesRow::serialize(data).unwrap();
    let query = serde_json::json!({"value": "00000000"});
    let result = BytesRow::filter_bytes(&bytes, query, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_bytes_filter_inclusion_match() {
    let data = vec![
        BytesRow { value: ByteBuf::from(b"hello".as_slice()) },
        BytesRow { value: ByteBuf::from(b"world".as_slice()) },
        BytesRow { value: ByteBuf::from(b"foo".as_slice()) },
    ];
    let bytes = BytesRow::serialize(data).unwrap();
    let query = serde_json::json!({"value": ["68656c6c6f", "666f6f"]}); // "hello", "foo" in hex
    let result = BytesRow::filter_bytes(&bytes, query, &[]);
    assert!(result.is_err());
}

#[test]
fn test_bytes_filter_invalid_hex() {
    let data = vec![BytesRow { value: ByteBuf::from(b"hello".as_slice()) }];
    let bytes = BytesRow::serialize(data).unwrap();
    let query = serde_json::json!({"value": "not-hex"});
    let result = BytesRow::filter_bytes(&bytes, query, &[]);
    assert!(result.is_err());
}
