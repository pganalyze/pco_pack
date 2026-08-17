use crate::filter_mask::*;
use bitvec::prelude::*;

fn mask_from_words(words: &[u64], len: usize) -> FilterMask {
    let mut bits = BitVec::<u64, Lsb0>::from_slice(words);
    bits.truncate(len);
    FilterMask(bits)
}

#[test]
fn any_set_full_range() {
    let mut m = FilterMask::ones(256);
    m.0.get_mut(0..64).unwrap().fill(false);
    assert!(m.any_set());
    m.0.get_mut(64..256).unwrap().fill(false);
    assert!(!m.any_set());
}

#[test]
fn any_set_empty_mask() {
    let m = FilterMask::ones(0);
    assert!(!m.any_set());
    assert!(!m.any_set_in_range(0..10));
}

#[test]
fn any_set_in_range_single_word() {
    let mut m = mask_from_words(&[0u64; 4], 256);
    m.0.get_mut(70..100).unwrap().fill(true);
    assert!(m.any_set_in_range(64..128));
    assert!(m.any_set_in_range(0..256));
    assert!(!m.any_set_in_range(0..64));
    assert!(!m.any_set_in_range(128..192));
    assert!(!m.any_set_in_range(300..400));
    assert!(!m.any_set_in_range(100..100));
    assert!(!m.any_set_in_range(100..50));
}

#[test]
fn any_set_in_range_crossing_word_boundary() {
    // Range spanning two words.
    assert!(!mask_from_words(&[0u64; 17], 1088).any_set_in_range(1000..1064));

    let mut m = mask_from_words(&[0u64; 17], 1088);
    m.0.get_mut(1000..1024).unwrap().fill(true);
    assert!(m.any_set_in_range(1000..1064));

    let mut m = mask_from_words(&[0u64; 17], 1088);
    m.0.get_mut(1024..1064).unwrap().fill(true);
    assert!(m.any_set_in_range(1000..1064));

    // Bits just outside the range must not count.
    let mut m = mask_from_words(&[0u64; 17], 1088);
    m.0.get_mut(999..1000).unwrap().fill(true);
    m.0.get_mut(1064..1065).unwrap().fill(true);
    assert!(!m.any_set_in_range(1000..1064));
}

#[test]
fn any_set_in_range_ignores_padding() {
    // 100 valid bits: word 1's bits 36..63 are padding.
    assert!(!mask_from_words(&[0u64, u64::MAX << 36], 100).any_set());
    assert!(mask_from_words(&[0u64, 1u64 << 35], 100).any_set());
}

#[test]
fn build_with_and_full_and_partial_words() {
    let values: Vec<i64> = (0..200).collect(); // 3 full words + 8
    let mut m = FilterMask::ones(200);
    m.build_with_and(&values, |&v| v % 2 == 0);
    assert_eq!(m.count_ones(), 100);
    let bits = m.as_bitslice();
    for i in 0..200 {
        assert_eq!(bits[i], i % 2 == 0, "row {i}");
    }
}

#[test]
fn build_with_and_preserves_cleared_bits() {
    // AND semantics: previously cleared bits stay cleared.
    let values: Vec<i64> = (0..10).collect();
    let mut m = FilterMask::ones(10);
    m.0.get_mut(3..4).unwrap().fill(false);
    m.build_with_and(&values, |_| true);
    assert_eq!(m.count_ones(), 9);
    assert!(!m.as_bitslice()[3]);
}

#[test]
fn build_with_and_empty_values_uses_default() {
    let mut m = FilterMask::ones(10);
    m.build_with_and::<i64, _>(&[], |_| false);
    assert_eq!(m.count_ones(), 0);
    let mut m = FilterMask::ones(10);
    m.build_with_and::<i64, _>(&[], |_| true);
    assert_eq!(m.count_ones(), 10);
}

#[test]
fn build_into_unaligned_within_single_word() {
    // Unaligned start within a single word.
    let values: Vec<i64> = (0..32).collect();
    let mut m = FilterMask::ones(128);
    m.build_into(5, &values, |&v| v % 2 == 0);
    let bits = m.as_bitslice();
    for i in 0..128 {
        let expected = if (5..37).contains(&i) { (i - 5) % 2 == 0 } else { true };
        assert_eq!(bits[i], expected, "row {i}");
    }
}

#[test]
fn build_into_unaligned_start_crossing_word() {
    let values: Vec<i64> = (0..64).collect();
    let mut m = FilterMask::ones(1088);
    m.build_into(1000, &values, |&v| v % 2 == 0);
    let bits = m.as_bitslice();
    for i in 0..1088 {
        let expected = if (1000..1064).contains(&i) { (i - 1000) % 2 == 0 } else { true };
        assert_eq!(bits[i], expected, "row {i}");
    }
}

#[test]
fn build_into_aligned_full_words() {
    let values: Vec<i64> = (0..128).collect();
    let mut m = FilterMask::ones(128);
    m.build_into(0, &values, |&v| v % 2 == 0);
    let bits = m.as_bitslice();
    for i in 0..128 {
        assert_eq!(bits[i], i % 2 == 0, "row {i}");
    }
    assert_eq!(m.count_ones(), 64);
}

#[test]
fn build_into_aligned_partial_last_word() {
    // One full word plus a partial last word.
    let values: Vec<i64> = (0..100).collect();
    let mut m = FilterMask::ones(100);
    m.build_into(0, &values, |&v| v % 2 == 0);
    let bits = m.as_bitslice();
    for i in 0..100 {
        assert_eq!(bits[i], i % 2 == 0, "row {i}");
    }
    assert_eq!(m.count_ones(), 50);
}

#[test]
fn build_into_aligned_nonzero_start() {
    // Aligned, nonzero start; excess values are truncated to the mask length.
    let values: Vec<i64> = (0..136).collect();
    let mut m = FilterMask::ones(256);
    m.build_into(128, &values, |&v| v % 4 == 0);
    let bits = m.as_bitslice();
    for i in 0..256 {
        let expected = if (128..256).contains(&i) { (i - 128) % 4 == 0 } else { true };
        assert_eq!(bits[i], expected, "row {i}");
    }
}

#[test]
fn build_into_preserves_cleared_bits() {
    // AND semantics: previously cleared bits stay cleared.
    let values: Vec<i64> = (0..128).collect();
    let mut m = FilterMask::ones(128);
    m.0.get_mut(70..90).unwrap().fill(false);
    m.build_into(0, &values, |_| true);
    assert_eq!(m.count_ones(), 108);
    for i in 70..90 {
        assert!(!m.as_bitslice()[i], "row {i}");
    }
}

#[test]
fn from_bool_slice_len_matches_input() {
    let m = FilterMask::from_bool_slice(&[true, false, true]);
    assert_eq!(m.as_bitslice().len(), 3);
    assert_eq!(m.count_ones(), 2);
    let m = FilterMask::from_bool_slice(&vec![true; 100]);
    assert_eq!(m.as_bitslice().len(), 100);
    assert_eq!(m.count_ones(), 100);
    assert_eq!(FilterMask::from_bool_slice(&[]).as_bitslice().len(), 0);
}

#[test]
fn from_bool_slice_word_content() {
    let mut input = vec![false; 130];
    input[0] = true;
    input[63] = true;
    input[64] = true;
    input[127] = true;
    input[129] = true;
    let m = FilterMask::from_bool_slice(&input);
    assert_eq!(m.count_ones(), 5);
    let raw = m.as_raw_slice();
    assert_eq!(raw[0], 1 | (1 << 63));
    assert_eq!(raw[1], 1 | (1 << 63));
    assert_eq!(raw[2], 1 << 1);
}
