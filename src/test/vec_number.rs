use crate as pco_pack;
use crate::PcoPack;

#[derive(PcoPack, Debug, PartialEq, Clone)]
struct VecI32 {
    values: Vec<i32>,
}

#[derive(PcoPack, Debug, PartialEq, Clone)]
struct VecF64 {
    values: Vec<f64>,
}

#[test]
fn vec_i32_roundtrip() {
    let data: Vec<Vec<i32>> = vec![vec![1, 2, 3], vec![], vec![42], vec![-1, 0, 1, 2]];
    let wrapped: Vec<VecI32> = data.into_iter().map(|v| VecI32 { values: v }).collect();
    let bytes = VecI32::serialize(wrapped.clone()).unwrap();
    let result = VecI32::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn vec_f64_roundtrip() {
    let data: Vec<Vec<f64>> = vec![vec![1.0, 2.0, 3.0], vec![], vec![std::f64::consts::PI], vec![-0.5, 0.0, 0.5]];
    let wrapped: Vec<VecF64> = data.into_iter().map(|v| VecF64 { values: v }).collect();
    let bytes = VecF64::serialize(wrapped.clone()).unwrap();
    let result = VecF64::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn empty_vec_numeric_column() {
    let wrapped: Vec<VecI32> = vec![];
    let bytes = VecI32::serialize(wrapped).unwrap();
    let result = VecI32::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filter_vec_i32_exact_match() {
    let data: Vec<VecI32> = vec![
        VecI32 { values: vec![1, 2, 3] },      // no match for 42
        VecI32 { values: vec![] },             // no match (empty)
        VecI32 { values: vec![42] },           // matches
        VecI32 { values: vec![-1, 0, 1, 42] }, // matches (contains 42 at end)
    ];
    let bytes = VecI32::serialize(data.clone()).unwrap();

    let result = VecI32::filter_bytes(&bytes, serde_json::json!({"values": 42}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], data[2]); // [42]
    assert_eq!(result[1], data[3]); // [-1, 0, 1, 42]
}

#[test]
fn filter_vec_i32_no_match() {
    let data: Vec<VecI32> =
        vec![VecI32 { values: vec![1, 2, 3] }, VecI32 { values: vec![] }, VecI32 { values: vec![42] }];
    let bytes = VecI32::serialize(data.clone()).unwrap();

    let result = VecI32::filter_bytes(&bytes, serde_json::json!({"values": 99}), &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn filter_vec_i32_inclusion() {
    // Filter for rows whose vec contains ANY of {10, -1}. Match if any element is in the list
    let data: Vec<VecI32> = vec![
        VecI32 { values: vec![1, 2, 3] },  // no match
        VecI32 { values: vec![] },         // no match (empty)
        VecI32 { values: vec![42] },       // no match
        VecI32 { values: vec![10, -5] },   // matches (contains 10)
        VecI32 { values: vec![-1, 0, 1] }, // matches (contains -1)
    ];
    let bytes = VecI32::serialize(data.clone()).unwrap();

    let result = VecI32::filter_bytes(&bytes, serde_json::json!({"values": [10, -1]}), &[]).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].values, vec![10, -5]);
    assert_eq!(result[1].values, vec![-1, 0, 1]);
}

#[test]
fn filter_vec_i32_range() {
    // Filter for rows whose vec contains ANY element in range [20..=60]
    let data: Vec<VecI32> = vec![
        VecI32 { values: vec![1, 2, 3] }, // no match (all < 20)
        VecI32 { values: vec![] },        // no match (empty)
        VecI32 { values: vec![42, 100] }, // matches (42 in range)
        VecI32 { values: vec![-5, 99] },  // no match (both outside [20..=60])
    ];
    let bytes = VecI32::serialize(data.clone()).unwrap();

    let result = VecI32::filter_bytes(&bytes, serde_json::json!({"values": {"start": 20, "end": 60}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].values, vec![42, 100]);
}

#[test]
fn filter_vec_f64_exact_match() {
    let data: Vec<VecF64> = vec![
        VecF64 { values: vec![1.0, 2.0, 3.0] },
        VecF64 { values: vec![] },
        VecF64 { values: vec![std::f64::consts::PI] },
    ];
    let bytes = VecF64::serialize(data.clone()).unwrap();

    let result = VecF64::filter_bytes(&bytes, serde_json::json!({"values": std::f64::consts::PI}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].values, vec![std::f64::consts::PI]);
}

#[test]
fn filter_vec_f64_range() {
    let data: Vec<VecF64> = vec![
        VecF64 { values: vec![1.0, 2.0] },
        VecF64 { values: vec![] },
        VecF64 { values: vec![3.5] },
        VecF64 { values: vec![10.0, -1.0] },
    ];
    let bytes = VecF64::serialize(data.clone()).unwrap();

    let result = VecF64::filter_bytes(&bytes, serde_json::json!({"values": {"start": 2.5, "end": 3.5}}), &[]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].values, vec![3.5]);
}

#[test]
fn filter_vec_i32_empty_rows_handled() {
    let data: Vec<VecI32> = vec![VecI32 { values: vec![] }, VecI32 { values: vec![] }];
    let bytes = VecI32::serialize(data.clone()).unwrap();

    let result = VecI32::filter_bytes(&bytes, serde_json::json!({"values": 100}), &[]).unwrap();
    assert!(result.is_empty());
}
