use crate::FallbackReader;
use crate::PcoSerde;
use crate::string::StringReader;
use std::io::Cursor;

fn legacy_compress<T: serde::Serialize>(items: &[T]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut encoder = zstd::stream::write::Encoder::new(&mut output, 3).unwrap();
    for item in items {
        rmp_serde::encode::write(&mut encoder, item).unwrap();
    }
    encoder.finish().unwrap().to_vec()
}

#[test]
fn string_fallback_deserialize_single() {
    let original = "hello world".to_string();
    let raw = legacy_compress(&[original.clone()]);
    let mut reader = <StringReader as FallbackReader>::read_fallback(&raw).unwrap();
    let popped = <String as PcoSerde>::get(&mut reader, 0).unwrap().unwrap();
    assert_eq!(popped, original);
}

#[test]
fn string_fallback_deserialize_empty() {
    let original = "".to_string();
    let raw = legacy_compress(&[original.clone()]);
    let mut reader = <StringReader as FallbackReader>::read_fallback(&raw).unwrap();
    let popped = <String as PcoSerde>::get(&mut reader, 0).unwrap().unwrap();
    assert_eq!(popped, original);
}

#[test]
fn string_fallback_deserialize_unicode() {
    let original = "unicode: 你好世界 🌍".to_string();
    let raw = legacy_compress(&[original.clone()]);
    let mut reader = <StringReader as FallbackReader>::read_fallback(&raw).unwrap();
    let popped = <String as PcoSerde>::get(&mut reader, 0).unwrap().unwrap();
    assert_eq!(popped, original);
}

#[test]
fn string_fallback_validate_bounds() {
    let original = "test".to_string();
    let raw = legacy_compress(&[original.clone()]);
    let mut reader = <StringReader as FallbackReader>::read_fallback(&raw).unwrap();
    assert_eq!(String::validate_bounds(&mut reader).unwrap(), Some(1));
}

#[test]
fn string_fallback_multiple_items() {
    let items = vec!["first".to_string(), "second".to_string(), "third".to_string()];
    let raw = legacy_compress(&items);
    let mut reader = <StringReader as FallbackReader>::read_fallback(&raw).unwrap();
    assert_eq!(String::validate_bounds(&mut reader).unwrap(), Some(3));
    for i in 0..3 {
        assert_eq!(<String as PcoSerde>::get(&mut reader, i).unwrap().unwrap(), items[i]);
    }
}

#[test]
fn string_normal_vs_fallback_equivalent() {
    let original = "test value".to_string();
    let normal_buf = String::write(vec![original.clone()], 0, chrono::Duration::zero()).unwrap();
    let fallback_buf = legacy_compress(&[original.clone()]);
    let mut normal_cursor = std::io::Cursor::new(normal_buf.as_slice());
    let normal_cols = <String as PcoSerde>::read(&mut normal_cursor, 0, chrono::Duration::zero()).unwrap();
    let mut normal_cols_mut = normal_cols;
    let normal_val = <String as PcoSerde>::get(&mut normal_cols_mut, 0).unwrap().unwrap();

    let fallback_cols = <StringReader as FallbackReader>::read_fallback(&fallback_buf).unwrap();
    let mut fallback_cols_mut = fallback_cols;
    let fallback_val = <String as PcoSerde>::get(&mut fallback_cols_mut, 0).unwrap().unwrap();

    assert_eq!(normal_val, fallback_val);
    assert_eq!(normal_val, original);
}

#[test]
fn string_read_falls_back_to_legacy() {
    let items = vec!["mountpoint1".to_string(), "mountpoint2".to_string()];
    let legacy_buf = legacy_compress(&items);

    let mut cursor = Cursor::new(legacy_buf.as_slice());
    let mut reader = <String as PcoSerde>::read(&mut cursor, 0, chrono::Duration::zero()).unwrap();
    assert_eq!(String::validate_bounds(&mut reader).unwrap(), Some(2));

    let mut reader_mut = reader;
    assert_eq!(<String as PcoSerde>::get(&mut reader_mut, 0).unwrap().unwrap(), "mountpoint1");
    assert_eq!(<String as PcoSerde>::get(&mut reader_mut, 1).unwrap().unwrap(), "mountpoint2");
}
