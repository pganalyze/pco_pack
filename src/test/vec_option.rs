use crate as pco_pack;
use crate::PcoPack;

#[derive(PcoPack, Debug, PartialEq, Clone)]
struct VecOptionI32 {
    values: Vec<Option<i32>>,
}

#[test]
fn vec_option_i32_roundtrip() {
    let data: Vec<Vec<Option<i32>>> = vec![vec![Some(1), None, Some(3)], vec![], vec![None, None], vec![Some(42)]];
    let wrapped: Vec<VecOptionI32> = data.into_iter().map(|v| VecOptionI32 { values: v }).collect();
    let bytes = VecOptionI32::serialize(wrapped.clone()).unwrap();
    let result = VecOptionI32::deserialize(&bytes).unwrap();
    assert_eq!(wrapped, result);
}

#[test]
fn empty_vec_option_column() {
    let wrapped: Vec<VecOptionI32> = vec![];
    let bytes = VecOptionI32::serialize(wrapped).unwrap();
    let result = VecOptionI32::deserialize(&bytes).unwrap();
    assert!(result.is_empty());
}
