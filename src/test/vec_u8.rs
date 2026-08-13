use crate as pco_pack;
use crate::PcoPack;

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
struct VecU8Row {
    data: Vec<u8>,
}

#[test]
fn vec_u8_roundtrip_basic() {
    let data = vec![VecU8Row { data: vec![0, 1, 2, 3] }, VecU8Row { data: vec![] }, VecU8Row { data: vec![42] }];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();
    let result = VecU8Row::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_u8_roundtrip_full_range() {
    let data = vec![VecU8Row { data: vec![0, 127, 128, 254, 255] }];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();
    let result = VecU8Row::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_u8_roundtrip_many_rows() {
    let data: Vec<VecU8Row> =
        (0..100).map(|i| VecU8Row { data: (0..(i % 20 + 1)).map(|j| (i * j) as u8).collect() }).collect();
    let bytes = VecU8Row::serialize(data.clone()).unwrap();
    let result = VecU8Row::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_u8_all_empty() {
    let data = vec![VecU8Row { data: vec![] }, VecU8Row { data: vec![] }];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();
    let result = VecU8Row::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_u8_filter_exact_match() {
    let data = vec![
        VecU8Row { data: vec![1, 2, 3] },    // no match for 42
        VecU8Row { data: vec![] },           // no match (empty)
        VecU8Row { data: vec![42] },         // matches
        VecU8Row { data: vec![10, 42, 50] }, // matches (contains 42)
    ];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();

    let result = VecU8Row::filter_bytes(&bytes, serde_json::json!({"data": 42}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].data, vec![42]);
    assert_eq!(result[1].data, vec![10, 42, 50]);
}

#[test]
fn vec_u8_filter_range() {
    let data = vec![
        VecU8Row { data: vec![1, 2, 3] },  // no match (all < 50)
        VecU8Row { data: vec![] },         // no match (empty)
        VecU8Row { data: vec![60, 100] },  // matches (both in [50..=80])
        VecU8Row { data: vec![200, 250] }, // no match (all > 80)
    ];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();

    let result = VecU8Row::filter_bytes(&bytes, serde_json::json!({"data": {"start": 50, "end": 80}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].data, vec![60, 100]);
}

#[test]
fn vec_u8_filter_inclusion() {
    let data = vec![
        VecU8Row { data: vec![1, 2, 3] },  // no match
        VecU8Row { data: vec![] },         // no match (empty)
        VecU8Row { data: vec![100, 50] },  // matches (contains 100)
        VecU8Row { data: vec![200, 255] }, // matches (contains 255)
    ];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();

    let result = VecU8Row::filter_bytes(&bytes, serde_json::json!({"data": [100, 255]}), &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn vec_u8_filter_no_match() {
    let data = vec![VecU8Row { data: vec![1, 2, 3] }, VecU8Row { data: vec![] }];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();

    let result = VecU8Row::filter_bytes(&bytes, serde_json::json!({"data": 99}), &[]).unwrap();
    assert!(result.is_empty());
}

#[derive(Debug, Clone, PartialEq, Default, PcoPack)]
#[pco_pack(chunk_size = 5)]
struct SmallChunkVecU8Row {
    data: Vec<u8>,
}

#[test]
fn vec_u8_chunk_boundary_roundtrip() {
    let data: Vec<SmallChunkVecU8Row> =
        (0..20).map(|i| SmallChunkVecU8Row { data: vec![i as u8, (i * 2) as u8] }).collect();

    let bytes = SmallChunkVecU8Row::serialize(data.clone()).unwrap();
    let result = SmallChunkVecU8Row::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}

#[test]
fn vec_u8_chunk_boundary_filter() {
    let data: Vec<SmallChunkVecU8Row> =
        (0..20).map(|i| SmallChunkVecU8Row { data: vec![i as u8, (i * 3) as u8] }).collect();

    let bytes = SmallChunkVecU8Row::serialize(data).unwrap();

    // Filter for rows containing value 15. Should match row with i=5 (data=[5,15]) and i=15 (data=[15,45]).
    let result = SmallChunkVecU8Row::filter_bytes(&bytes, serde_json::json!({"data": 15}), &[]).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn vec_u8_large_single_row() {
    let data = vec![VecU8Row { data: (0..10_000).map(|i| i as u8).collect() }];
    let bytes = VecU8Row::serialize(data.clone()).unwrap();
    let result = VecU8Row::deserialize(&bytes).unwrap();
    assert_eq!(data, result);
}
